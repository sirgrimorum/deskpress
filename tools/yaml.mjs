// A YAML reader for the subset a pack uses. No dependencies, on purpose: this file and
// validate.mjs run unchanged on a desk with node and inside the app, so a pack that passes in
// one place passes in the other.
//
// Supported: block mappings, block sequences, single line flow sequences and mappings, literal
// and folded block scalars with chomping, single and double quoted scalars, comments, and the
// plain scalar coercions (null, booleans, numbers). Dates stay strings, which is what every
// consumer here wants.
//
// A flow collection may wrap onto the following lines, which is what a long block wants.
//
// Not supported, and the reader says so instead of guessing: tabs for indentation, anchors and
// aliases, more than one document in a file, and tags. A pack is one document per file.

export class YamlError extends Error {
  constructor(message, line) {
    super(line ? `line ${line}: ${message}` : message);
    this.line = line || null;
  }
}

const TRUE = new Set(['true', 'True', 'TRUE', 'yes', 'Yes', 'YES', 'on', 'On']);
const FALSE = new Set(['false', 'False', 'FALSE', 'no', 'No', 'NO', 'off', 'Off']);
const NULL = new Set(['', '~', 'null', 'Null', 'NULL']);
const NUMBER = /^[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?$/;

export function parse(text) {
  const raw = String(text).replace(/^﻿/, '').replace(/\r\n?/g, '\n').split('\n');
  const doc = new Reader(raw);
  doc.openDocument();
  doc.skipBlanks();
  if (doc.eof()) return null;
  const value = doc.node(0);
  doc.skipBlanks();
  if (!doc.eof()) throw new YamlError('content left over after the document', doc.lineNo());
  return value;
}

class Reader {
  constructor(lines) {
    this.lines = lines;
    this.i = 0;
  }

  eof() { return this.i >= this.lines.length; }
  lineNo() { return this.i + 1; }

  // A single leading "---" opens this document. Any later one starts a second, which is an error.
  openDocument() {
    while (this.i < this.lines.length) {
      const trimmed = this.lines[this.i].trim();
      if (trimmed === '' || trimmed.startsWith('#')) { this.i++; continue; }
      if (trimmed === '---') this.i++;
      return;
    }
  }

  skipBlanks() {
    while (this.i < this.lines.length) {
      const line = this.lines[this.i];
      const trimmed = line.trim();
      if (trimmed === '' || trimmed.startsWith('#')) { this.i++; continue; }
      if (trimmed === '---') {
        throw new YamlError('a second document starts here. A pack is one document per file, and merging two of them would lose keys quietly', this.lineNo());
      }
      if (trimmed === '...') {
        const more = this.lines.slice(this.i + 1).some((l) => l.trim() !== '' && !l.trim().startsWith('#'));
        if (more) throw new YamlError('the document ends here, but the file keeps going', this.lineNo());
        this.i = this.lines.length;
        continue;
      }
      if (/^\s*\t/.test(line)) {
        throw new YamlError('tab used for indentation. YAML needs spaces', this.lineNo());
      }
      return;
    }
  }

  // The current line, split into its indentation and its content.
  head() {
    const line = this.lines[this.i];
    const indent = line.length - line.replace(/^ +/, '').length;
    return { indent, text: line.slice(indent).replace(/\s+$/, ''), line };
  }

  node(minIndent) {
    this.skipBlanks();
    if (this.eof()) return null;
    const { indent, text } = this.head();
    if (indent < minIndent) return null;
    if (isSeqEntry(text)) return this.sequence(indent);
    return this.mapping(indent);
  }

  sequence(indent) {
    const out = [];
    for (;;) {
      this.skipBlanks();
      if (this.eof()) break;
      const { indent: at, text } = this.head();
      if (at !== indent || !isSeqEntry(text)) break;
      const rest = text.replace(/^-\s*/, '');
      const restColumn = indent + (text.length - rest.length);
      if (rest === '') {
        this.i++;
        out.push(this.node(indent + 1));
      } else if (isMappingStart(rest)) {
        // "- id: coast" opens a mapping whose other keys sit under the text, not under the dash.
        // Blanking the dash lets the mapping parser see a normal line.
        this.lines[this.i] = ' '.repeat(restColumn) + rest;
        out.push(this.mapping(restColumn));
      } else if (rest === '|' || rest === '>' || /^[|>][-+]?\d*$/.test(rest)) {
        this.i++;
        out.push(this.blockScalar(rest, indent));
      } else {
        const line = this.lineNo();
        this.i++;
        out.push(scalar(this.joinFlow(rest, line), line));
      }
    }
    return out;
  }

  mapping(indent) {
    const out = {};
    for (;;) {
      this.skipBlanks();
      if (this.eof()) break;
      const { indent: at, text } = this.head();
      if (at < indent) break;
      if (at > indent) throw new YamlError('unexpected indentation', this.lineNo());
      if (isSeqEntry(text)) break;
      const split = splitKey(text);
      if (!split) throw new YamlError(`expected "key: value", found ${JSON.stringify(text)}`, this.lineNo());
      const key = checkKey(unquote(split.key, this.lineNo()), this.lineNo());
      if (Object.prototype.hasOwnProperty.call(out, key)) {
        throw new YamlError(`duplicate key ${JSON.stringify(key)}`, this.lineNo());
      }
      const rest = split.rest;
      if (/^[|>][-+]?\d*$/.test(rest)) {
        this.i++;
        out[key] = this.blockScalar(rest, indent);
      } else if (rest === '') {
        this.i++;
        out[key] = this.childOf(indent);
      } else {
        const line = this.lineNo();
        this.i++;
        out[key] = scalar(this.joinFlow(rest, line), line);
      }
    }
    return out;
  }

  // The value of a key that had nothing after the colon: a nested block, a sequence written at
  // the key's own indentation (both styles are common), or nothing at all.
  childOf(indent) {
    this.skipBlanks();
    if (this.eof()) return null;
    const { indent: at, text } = this.head();
    if (at === indent && isSeqEntry(text)) return this.sequence(indent);
    if (at > indent) return this.node(indent + 1);
    return null;
  }

  // A flow collection is allowed to wrap: keep taking lines until the brackets balance.
  joinFlow(text, line) {
    if (text[0] !== '[' && text[0] !== '{') return text;
    let joined = text;
    while (!flowComplete(joined)) {
      if (this.i >= this.lines.length) {
        throw new YamlError('a flow collection was never closed', line);
      }
      const next = this.lines[this.i].trim();
      this.i++;
      if (next === '' || next.startsWith('#')) continue;
      joined += ` ${next}`;
    }
    return joined;
  }

  blockScalar(header, indent) {
    const folded = header[0] === '>';
    const chomp = /[-+]/.test(header) ? header.match(/[-+]/)[0] : 'clip';
    const forced = header.match(/\d/) ? Number(header.match(/\d/)[0]) : null;
    const body = [];
    let base = forced === null ? null : indent + forced;
    while (this.i < this.lines.length) {
      const line = this.lines[this.i];
      if (line.trim() === '') { body.push(''); this.i++; continue; }
      const at = line.length - line.replace(/^ +/, '').length;
      if (at <= indent) break;
      if (base === null) base = at;
      if (at < base) break;
      body.push(line.slice(base).replace(/\s+$/, ''));
      this.i++;
    }
    while (body.length && body[body.length - 1] === '') body.pop();
    let text;
    if (!folded) {
      text = body.join('\n');
    } else {
      const parts = [];
      for (const line of body) {
        if (line === '') parts.push('\n');
        else if (parts.length && parts[parts.length - 1] !== '\n') parts.push(' ', line);
        else parts.push(line);
      }
      text = parts.join('').replace(/\n /g, '\n');
    }
    if (chomp === '-') return text;
    return text + '\n';
  }
}

// True when every bracket and quote opened in the text is closed again.
function flowComplete(text) {
  let quote = null;
  let depth = 0;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quote) {
      if (c === quote) quote = null;
      else if (c === '\\' && quote === '"') i++;
      continue;
    }
    if (isQuoteStart(text, i)) { quote = c; continue; }
    if (c === '[' || c === '{') depth++;
    else if (c === ']' || c === '}') depth--;
  }
  return quote === null && depth === 0;
}

// An apostrophe inside a word is a letter, not a quote: L'Hospitalet is one plain scalar.
function isQuoteStart(text, i) {
  const c = text[i];
  if (c !== '"' && c !== "'") return false;
  const prev = text[i - 1];
  return prev === undefined || /[\s,[{:]/.test(prev);
}

// Assigning "__proto__" with brackets runs the prototype setter, so the key would vanish instead
// of rendering as a card like every other unknown key.
function checkKey(key, line) {
  if (key === '__proto__') {
    throw new YamlError('"__proto__" cannot be a key: it would disappear instead of rendering', line);
  }
  return key;
}

function isSeqEntry(text) {
  return text === '-' || /^-\s/.test(text);
}

function isMappingStart(text) {
  if (text.startsWith('[') || text.startsWith('{')) return false;
  return splitKey(text) !== null;
}

// Finds the colon that ends a key: the first ": " or trailing ":" that is not inside quotes or
// inside a flow collection.
function splitKey(text) {
  let quote = null;
  let depth = 0;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quote) {
      if (c === quote) quote = null;
      else if (c === '\\' && quote === '"') i++;
      continue;
    }
    if (isQuoteStart(text, i)) { quote = c; continue; }
    if (c === '[' || c === '{') { depth++; continue; }
    if (c === ']' || c === '}') { depth--; continue; }
    if (c === '#' && i > 0 && /\s/.test(text[i - 1]) && depth === 0) return null;
    if (c === ':' && depth === 0) {
      const next = text[i + 1];
      if (next === undefined || next === ' ') {
        const key = text.slice(0, i).trim();
        if (key === '') return null;
        return { key, rest: stripComment(text.slice(i + 1).trim()) };
      }
    }
  }
  return null;
}

function stripComment(text) {
  let quote = null;
  let depth = 0;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quote) {
      if (c === quote) quote = null;
      else if (c === '\\' && quote === '"') i++;
      continue;
    }
    if (isQuoteStart(text, i)) { quote = c; continue; }
    if (c === '[' || c === '{') { depth++; continue; }
    if (c === ']' || c === '}') { depth--; continue; }
    if (c === '#' && depth === 0 && (i === 0 || /\s/.test(text[i - 1]))) {
      return text.slice(0, i).replace(/\s+$/, '');
    }
  }
  return text;
}

function unquote(text, line) {
  if (text.length > 1 && text[0] === '"' && text[text.length - 1] === '"') {
    try {
      return JSON.parse(text);
    } catch {
      throw new YamlError(`${text} has an escape this reader does not know. Inside double quotes a backslash starts \n, \t, \\" or \\`, line);
    }
  }
  if (text.length > 1 && text[0] === "'" && text[text.length - 1] === "'") {
    return text.slice(1, -1).replace(/''/g, "'");
  }
  return text;
}

function scalar(text, line) {
  const t = stripComment(text).trim();
  if (t.startsWith('[') || t.startsWith('{')) return flow(t, line);
  if (t[0] === '"' || t[0] === "'") {
    if (t.length < 2 || t[t.length - 1] !== t[0]) throw new YamlError('unterminated quoted string', line);
    return unquote(t, line);
  }
  if (NULL.has(t)) return null;
  if (TRUE.has(t)) return true;
  if (FALSE.has(t)) return false;
  if (NUMBER.test(t)) return Number(t);
  return t;
}

// A flow collection on one line: ["09:20", "text", {type: visit}]
function flow(text, line) {
  const cursor = { s: text, i: 0, line };
  const value = flowValue(cursor);
  skipSpace(cursor);
  if (cursor.i < cursor.s.length) {
    throw new YamlError(`unexpected ${JSON.stringify(cursor.s[cursor.i])} after a flow collection. A flow collection has to fit on one line`, line);
  }
  return value;
}

function skipSpace(c) { while (c.i < c.s.length && /\s/.test(c.s[c.i])) c.i++; }

function flowValue(c) {
  skipSpace(c);
  const ch = c.s[c.i];
  if (ch === '[') return flowSeq(c);
  if (ch === '{') return flowMap(c);
  return scalar(flowPlain(c), c.line);
}

function flowSeq(c) {
  c.i++;
  const out = [];
  for (;;) {
    skipSpace(c);
    if (c.i >= c.s.length) throw new YamlError('a flow sequence was never closed with "]"', c.line);
    if (c.s[c.i] === ']') { c.i++; return out; }
    out.push(flowValue(c));
    skipSpace(c);
    if (c.s[c.i] === ',') { c.i++; continue; }
    if (c.s[c.i] === ']') { c.i++; return out; }
    throw new YamlError(`expected "," or "]" in a flow sequence, found ${JSON.stringify(c.s[c.i] || 'end of line')}`, c.line);
  }
}

function flowMap(c) {
  c.i++;
  const out = {};
  for (;;) {
    skipSpace(c);
    if (c.i >= c.s.length) throw new YamlError('a flow mapping was never closed with "}"', c.line);
    if (c.s[c.i] === '}') { c.i++; return out; }
    const key = checkKey(unquote(flowPlain(c, true).trim(), c.line), c.line);
    skipSpace(c);
    if (c.s[c.i] === ',' || c.s[c.i] === '}') {
      // YAML allows a key with no value here. It is almost always an unquoted value with a comma
      // in it, so the validator reports it as a warning rather than letting it vanish.
      out[key] = null;
    } else {
      if (c.s[c.i] !== ':') throw new YamlError(`expected ":" after the key ${JSON.stringify(key)} in a flow mapping`, c.line);
      c.i++;
      out[key] = flowValue(c);
    }
    skipSpace(c);
    if (c.s[c.i] === ',') { c.i++; continue; }
    if (c.s[c.i] === '}') { c.i++; return out; }
    throw new YamlError(`expected "," or "}" in a flow mapping, found ${JSON.stringify(c.s[c.i] || 'end of line')}`, c.line);
  }
}

// Reads a quoted or plain scalar up to the next separator.
function flowPlain(c, isKey = false) {
  skipSpace(c);
  const start = c.i;
  const q = c.s[c.i];
  if (q === '"' || q === "'") {
    c.i++;
    while (c.i < c.s.length) {
      if (c.s[c.i] === '\\' && q === '"') { c.i += 2; continue; }
      if (c.s[c.i] === q) { c.i++; return c.s.slice(start, c.i); }
      c.i++;
    }
    throw new YamlError('unterminated quoted string inside a flow collection', c.line);
  }
  while (c.i < c.s.length && !',]}'.includes(c.s[c.i]) && !(isKey && c.s[c.i] === ':')) c.i++;
  return c.s.slice(start, c.i);
}
