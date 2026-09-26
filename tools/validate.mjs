#!/usr/bin/env node
// The validator. The same file runs on a desk with node and inside the app, so a pack that passes
// here passes there. No dependencies, and no exceptions thrown for a bad pack: every problem is
// collected and reported, because the person fixing it wants the whole list, once.
//
//   node tools/validate.mjs <pack directory or pack.yaml>
//
// Errors block the load: the app will not pretend. Warnings load and show a line.

import { parse } from './yaml.mjs';

const TYPES = ['visit', 'train', 'driving', 'walking', 'meal', 'event', 'flight', 'parking',
  'lodging', 'night', 'morning', 'transfer', 'free'];
const SEVERITIES = ['critical', 'high', 'medium', 'low'];
const TOKENS = ['paper', 'ink', 'ink-muted', 'card', 'card-line', 'line', 'rule', 'soft', 'alert',
  'action-bg', 'action-ink', 'highlight-bg', 'highlight-ink', 'highlight-line', 'highlight-text',
  'chip-bg', 'chip-ink', 'bar-bg', 'bar-ink', 'bar-muted',
  'missing-fill', 'missing-border', 'missing-text'];
const PAIRS = [
  ['ink', 'paper'], ['ink-muted', 'paper'], ['ink', 'card'], ['ink-muted', 'card'],
  ['action-ink', 'action-bg'], ['highlight-ink', 'highlight-bg'], ['highlight-text', 'highlight-bg'],
  ['chip-ink', 'chip-bg'], ['bar-ink', 'bar-bg'], ['bar-muted', 'bar-bg'],
  ['alert', 'paper'], ['alert', 'card'], ['missing-text', 'missing-fill'],
];
const ID = /^[a-z_][a-z0-9_]*$/;
const SLUG = /^[a-z0-9]+(-[a-z0-9]+)*$/;
const DATE = /^\d{4}-\d{2}-\d{2}$/;
const TIME = /^([01]\d|2[0-3]):[0-5]\d$/;
const STAMP = /^\d{4}-\d{2}-\d{2}T([01]\d|2[0-3]):[0-5]\d$/;
const TO_CONFIRM = /\[(to confirm|por confirmar)\]/i;

export function validatePack({ manifest, content, theme } = {}) {
  const r = new Report();
  if (!isMap(manifest)) {
    r.error('pack.yaml', 'the manifest is missing or is not a mapping');
    return r.done();
  }
  const head = manifest.pack;
  if (!isMap(head)) {
    r.error('pack.yaml', 'the manifest needs a "pack:" block with id, name, language, timezone and content');
    return r.done();
  }

  // 1. Skeleton of the manifest.
  for (const key of ['id', 'name', 'language', 'timezone', 'content']) {
    if (head[key] === undefined || head[key] === null || head[key] === '') {
      r.error(`pack.${key}`, 'required in the manifest');
    }
  }
  if (head.id && !SLUG.test(String(head.id))) {
    r.error('pack.id', `${JSON.stringify(head.id)} is not a slug. Calendar event ids derive from it, so it has to be stable and plain: lowercase, digits and single dashes`);
  }
  if (head.timezone && !/^[A-Za-z_]+\/[A-Za-z_+-]+(\/[A-Za-z_+-]+)?$|^UTC$/.test(String(head.timezone))) {
    r.error('pack.timezone', `${JSON.stringify(head.timezone)} is not an IANA timezone name, like Europe/Madrid`);
  }
  if (head.language && !/^[a-z]{2,3}(-[A-Za-z0-9]{2,8})*$/.test(String(head.language))) {
    r.error('pack.language', `${JSON.stringify(head.language)} is not a language tag, like es or pt-BR`);
  }

  const map = isMap(manifest.keymap) ? manifest.keymap : {};
  const read = accessor(map);
  const conventions = isMap(manifest.conventions) ? manifest.conventions : {};
  for (const key of ['alert_prefixes', 'hidden_prefixes']) {
    if (conventions[key] !== undefined && !Array.isArray(conventions[key])) {
      r.error(`conventions.${key}`, 'has to be a list of prefixes');
    }
  }

  if (!isMap(content)) {
    r.error('content', 'the content file is missing or is not a mapping');
    return r.done();
  }

  const days = rootOf(content, map, 'days');
  const places = rootOf(content, map, 'places');
  const people = rootOf(content, map, 'people');
  const alerts = rootOf(content, map, 'alerts');
  const documents = rootOf(content, map, 'documents');

  if (days === undefined) {
    r.error('days', `no days in the content. If this pack calls them something else, map it: keymap.root.days`);
    return r.done();
  }
  if (!Array.isArray(days)) {
    r.error('days', 'has to be a list, one entry per day');
    return r.done();
  }

  // People first: days and blocks point at them.
  const personIds = new Set();
  const kids = new Set();
  if (people !== undefined) {
    if (!Array.isArray(people)) r.error('people', 'has to be a list');
    else people.forEach((p, i) => {
      const at = `people[${i}]`;
      if (!isMap(p)) { r.error(at, 'has to be a mapping'); return; }
      const id = p.id;
      if (!id) r.error(at, 'a person needs an id');
      else if (!ID.test(String(id))) r.error(`${at}.id`, `${JSON.stringify(id)} is not an id: lowercase letters, digits and underscores`);
      else if (personIds.has(id)) r.error(`${at}.id`, `${JSON.stringify(id)} is used twice`);
      else personIds.add(String(id));
      if (!read(p, 'person', 'name')) r.warn(at, 'a person with no name shows up as their id');
      const adult = read(p, 'person', 'adult');
      if (adult === false && id) kids.add(String(id));
      const family = str(read(p, 'person', 'theme'));
      if (family && isMap(theme) && isMap(theme.themes)) {
        const known = Object.keys(theme.themes).some((t) => t === family || t.startsWith(`${family}-`));
        if (!known) r.error(`${at}.theme`, `${JSON.stringify(family)} is not a theme in the theme file`);
      }
    });
  }

  // Places next, for the same reason.
  const placeIds = new Set();
  if (places !== undefined) {
    if (!isMap(places)) {
      r.error('places', 'has to be a mapping of place id to place');
    } else {
      for (const [id, place] of Object.entries(places)) {
        const at = `places.${id}`;
        if (!ID.test(id)) r.error(at, `${JSON.stringify(id)} is not an id: lowercase letters, digits and underscores`);
        placeIds.add(id);
        if (!isMap(place)) { r.error(at, 'has to be a mapping'); continue; }
        if (!read(place, 'place', 'name')) r.warn(at, 'no name, so the screen shows the id with its underscores turned into spaces');
        const coords = read(place, 'place', 'at');
        if (coords === undefined) {
          r.warn(at, 'no coordinates, so this place cannot become a geofence and the app has to trust the plan about where you are');
        } else if (!isMap(coords)) {
          r.error(`${at}.at`, 'has to be a mapping with lat, lon and radius_m');
        } else {
          const { lat, lon } = coords;
          const radius = coords.radius_m;
          if (typeof lat !== 'number' || lat < -90 || lat > 90) r.error(`${at}.at.lat`, `${JSON.stringify(lat)} is not a latitude`);
          if (typeof lon !== 'number' || lon < -180 || lon > 180) r.error(`${at}.at.lon`, `${JSON.stringify(lon)} is not a longitude`);
          if (radius !== undefined && (typeof radius !== 'number' || radius < 25 || radius > 20000)) {
            r.error(`${at}.at.radius_m`, `${JSON.stringify(radius)} is not a radius in metres between 25 and 20000`);
          }
        }
        const verified = read(place, 'place', 'verified');
        if (verified !== undefined && !DATE.test(String(verified))) {
          r.error(`${at}.verified`, `${JSON.stringify(verified)} is not a date, YYYY-MM-DD`);
        }
        const during = read(place, 'place', 'during');
        if (during !== undefined && !isMap(during)) r.error(`${at}.during`, 'has to be a mapping');
        const type = isMap(during) ? read(during, 'block', 'type') : undefined;
        if (type !== undefined) checkType(r, `${at}.during.type`, type, map);
        const parking = read(place, 'place', 'parking');
        if (parking !== undefined && !isMap(parking)) r.error(`${at}.parking`, 'has to be a mapping');
        // Points sit on the place, or inside during, where many packs already keep them.
        const points = read(place, 'place', 'points')
          ?? (isMap(during) ? read(during, 'place', 'points') : undefined);
        if (points !== undefined) {
          if (!Array.isArray(points)) r.error(`${at}.points`, 'has to be a list, in the order you would walk it');
          else {
            const seen = new Set();
            points.forEach((pt, i) => {
              const pat = `${at}.points[${i}]`;
              if (!isMap(pt)) { r.error(pat, 'has to be a mapping'); return; }
              if (!pt.id) r.warn(pat, 'a point with no id cannot be linked to');
              else if (seen.has(pt.id)) r.error(`${pat}.id`, `${JSON.stringify(pt.id)} is used twice in this place`);
              else seen.add(pt.id);
              if (!read(pt, 'point', 'name')) r.error(pat, 'a point needs a name');
              if (kids.size && !read(pt, 'point', 'for_kids')) {
                r.warn(pat, 'nothing written for a kid here, so this point is hidden in kid mode');
              }
            });
          }
        }
        walkFree(r, at, place, conventions, ['name', 'kind', 'at', 'safe', 'during', 'parking', 'points', 'verified'], map, 'place');
      }
    }
  }

  // 2 to 6. The days.
  const dayByDate = new Map();
  const optionsByDate = new Map();
  days.forEach((day, i) => {
    const label = `days[${i}]`;
    if (!isMap(day)) { r.error(label, 'has to be a mapping'); return; }
    const date = str(read(day, 'day', 'date'));
    const at = date ? `days.${date}` : label;
    if (!date) r.error(label, 'a day needs a date');
    else if (!DATE.test(date) || !realDate(date)) r.error(`${at}.date`, `${JSON.stringify(date)} is not a real date, YYYY-MM-DD`);
    else if (dayByDate.has(date)) r.error(`${at}.date`, `${date} appears twice. Two entries for one day means the reader has to guess`);
    else dayByDate.set(date, day);
    if (!read(day, 'day', 'title')) r.error(at, 'a day needs a title: it is what the agenda shows');

    const who = read(day, 'day', 'who');
    if (Array.isArray(who)) {
      who.forEach((id) => { if (!personIds.has(String(id))) r.warn(`${at}.who`, `${JSON.stringify(id)} is not one of the people in this pack`); });
    }

    const blocks = read(day, 'day', 'blocks');
    const fixed = read(day, 'day', 'fixed');
    const options = read(day, 'day', 'options');
    if (blocks === undefined && fixed === undefined && options === undefined) {
      r.warn(at, 'no blocks and no options, so this day has nothing to show');
    }
    if (blocks !== undefined) checkBlocks(r, `${at}.blocks`, blocks, { read, map, placeIds, personIds, conventions });
    if (fixed !== undefined) checkBlocks(r, `${at}.fixed`, fixed, { read, map, placeIds, personIds, conventions });

    if (options !== undefined) {
      if (!Array.isArray(options)) {
        r.error(`${at}.options`, 'has to be a list of closed alternatives');
      } else {
        if (options.length < 2) r.error(`${at}.options`, 'one option is not an option. Either two closed plans, or plain blocks');
        const ids = new Set();
        let recommended = 0;
        options.forEach((opt, j) => {
          const oat = `${at}.options[${j}]`;
          if (!isMap(opt)) { r.error(oat, 'has to be a mapping'); return; }
          if (!opt.id) r.error(oat, 'an option needs an id: the choice is stored on the device under it');
          else if (!ID.test(String(opt.id))) r.error(`${oat}.id`, `${JSON.stringify(opt.id)} is not an id: lowercase letters, digits and underscores`);
          else if (ids.has(opt.id)) r.error(`${oat}.id`, `${JSON.stringify(opt.id)} is used twice on this day`);
          else ids.add(String(opt.id));
          if (!read(opt, 'option', 'name')) r.error(oat, 'an option needs a name: it is what the suggestion screen puts on the button');
          if (read(opt, 'option', 'recommended') === true) recommended++;
          const obs = read(opt, 'option', 'blocks');
          if (obs === undefined) r.error(oat, 'an option with no blocks is an idea, not an option');
          else checkBlocks(r, `${oat}.blocks`, obs, { read, map, placeIds, personIds, conventions });
        });
        if (recommended === 0) r.warn(`${at}.options`, 'no option is recommended, so until somebody chooses the app has nothing to behave as');
        if (recommended > 1) r.error(`${at}.options`, `${recommended} options are recommended. Only one can be`);
        if (date) optionsByDate.set(date, ids);
      }
      const decision = read(day, 'day', 'decision');
      if (!isMap(decision)) {
        r.error(at, 'a day with options needs a decision block: when the app asks, and what that answer decides');
      } else {
        const when = str(read(decision, 'decision', 'when'));
        if (!when) r.error(`${at}.decision.when`, 'required: the day the app asks, once');
        else if (!DATE.test(when) || !realDate(when)) r.error(`${at}.decision.when`, `${JSON.stringify(when)} is not a real date`);
        else if (date && when > date) r.error(`${at}.decision.when`, `${when} is after the day it decides (${date}). The question has to be asked before it stops being a question`);
        const atTime = str(read(decision, 'decision', 'at'));
        if (atTime && !TIME.test(atTime)) r.error(`${at}.decision.at`, `${JSON.stringify(atTime)} is not a time, HH:MM on a 24 hour clock`);
        const decides = read(decision, 'decision', 'decides');
        if (decides !== undefined && !Array.isArray(decides)) r.error(`${at}.decision.decides`, 'has to be a list of dates');
      }
    }

    walkFree(r, at, day, conventions, ['date', 'title', 'who', 'blocks', 'fixed', 'options', 'decision'], map, 'day');
  });

  // 5 and 6, second pass: cross day references, now that every day is known.
  days.forEach((day) => {
    if (!isMap(day)) return;
    const date = str(read(day, 'day', 'date')) || '?';
    const decision = read(day, 'day', 'decision');
    if (isMap(decision) && Array.isArray(read(decision, 'decision', 'decides'))) {
      for (const other of read(decision, 'decision', 'decides')) {
        if (!dayByDate.has(str(other))) {
          r.error(`days.${date}.decision.decides`, `${JSON.stringify(str(other))} is not a day in this pack`);
        }
      }
    }
    const options = read(day, 'day', 'options');
    if (!Array.isArray(options)) return;
    options.forEach((opt, j) => {
      if (!isMap(opt) || opt.requires === undefined) return;
      const rat = `days.${date}.options[${j}].requires`;
      const req = opt.requires;
      if (!isMap(req)) { r.error(rat, 'has to be a mapping with date and option'); return; }
      const rdate = str(req.date);
      if (!dayByDate.has(rdate)) { r.error(rat, `${JSON.stringify(rdate)} is not a day in this pack`); return; }
      const ids = optionsByDate.get(rdate);
      if (!ids) r.error(rat, `${rdate} has no options, so nothing on it can be required`);
      else if (!ids.has(str(req.option))) r.error(rat, `${JSON.stringify(str(req.option))} is not an option on ${rdate}. It has ${[...ids].join(', ')}`);
    });
  });

  // The alerts.
  if (alerts !== undefined) {
    if (!Array.isArray(alerts)) r.error('alerts', 'has to be a list');
    else {
      const ids = new Set();
      alerts.forEach((a, i) => {
        const at = `alerts[${i}]`;
        if (!isMap(a)) { r.error(at, 'has to be a mapping'); return; }
        if (!a.id) r.error(at, 'an alert needs an id');
        else if (ids.has(a.id)) r.error(`${at}.id`, `${JSON.stringify(a.id)} is used twice`);
        else ids.add(a.id);
        if (!read(a, 'alert', 'title')) r.error(at, 'an alert needs a title: at critical severity it is the line above the answer');
        const sev = read(a, 'alert', 'severity');
        const canon = canonValue(map, 'severity', sev);
        if (sev === undefined) r.error(at, `an alert needs a severity: ${SEVERITIES.join(', ')}`);
        else if (!SEVERITIES.includes(canon)) {
          r.error(`${at}.severity`, `${JSON.stringify(sev)} is not a severity. Use ${SEVERITIES.join(', ')}, or map your own words in keymap.values.severity`);
        }
        for (const key of ['at', 'notify_from']) {
          const v = str(read(a, 'alert', key));
          if (v && !STAMP.test(v) && !DATE.test(v)) {
            r.error(`${at}.${key}`, `${JSON.stringify(v)} is not a date or a timestamp, YYYY-MM-DD or YYYY-MM-DDTHH:MM`);
          }
        }
        const time = str(read(a, 'alert', 'time'));
        if (time && !TIME.test(time)) r.error(`${at}.time`, `${JSON.stringify(time)} is not a time, HH:MM`);
      });
      const critical = alerts.filter((a) => isMap(a) && canonValue(map, 'severity', read(a, 'alert', 'severity')) === 'critical').length;
      if (critical > 6) r.warn('alerts', `${critical} alerts are critical. Severity is a budget: when everything is critical, the real one gets swiped past too`);
    }
  }

  // The documents.
  if (documents !== undefined) {
    if (!Array.isArray(documents)) r.error('documents', 'has to be a list');
    else {
      const docIds = new Set();
      documents.forEach((d, i) => {
        const at = `documents[${i}]`;
        if (!isMap(d)) { r.error(at, 'has to be a mapping'); return; }
        if (!d.id) r.error(at, 'a document needs an id');
        else if (!ID.test(String(d.id))) r.error(`${at}.id`, `${JSON.stringify(d.id)} is not an id: lowercase letters, digits and underscores`);
        else if (docIds.has(d.id)) r.error(`${at}.id`, `${JSON.stringify(d.id)} is used twice`);
        else docIds.add(d.id);
        if (!read(d, 'document', 'title')) r.error(at, 'a document needs a title');
        if (!read(d, 'document', 'file')) r.error(at, 'a document needs a file: it is the thing somebody at a counter is asking for');
        const owner = str(read(d, 'document', 'for'));
        if (owner && personIds.size && !personIds.has(owner)) r.error(`${at}.for`, `${JSON.stringify(owner)} is not one of the people in this pack`);
        const fields = read(d, 'document', 'fields');
        if (fields !== undefined && !isMap(fields)) r.error(`${at}.fields`, 'has to be a mapping of label to value');
      });
    }
  }

  // 7. The theme.
  if (theme !== undefined && theme !== null) {
    if (!isMap(theme) || !isMap(theme.themes)) {
      r.error('theme', 'a theme file needs a "themes:" mapping of theme id to theme');
    } else {
      const ids = Object.keys(theme.themes);
      if (theme.default && !ids.includes(String(theme.default))) {
        r.error('theme.default', `${JSON.stringify(theme.default)} is not one of the themes: ${ids.join(', ')}`);
      }
      for (const [id, t] of Object.entries(theme.themes)) {
        const at = `theme.themes.${id}`;
        if (!SLUG.test(id)) r.error(at, `${JSON.stringify(id)} is not a slug`);
        if (!isMap(t)) { r.error(at, 'has to be a mapping'); continue; }
        if (!t.name) r.error(at, 'a theme needs a name: it is what a person sees on the handoff screen');
        if (t.mode !== 'light' && t.mode !== 'dark') r.error(`${at}.mode`, 'has to be light or dark');
        if (!isMap(t.colors)) { r.error(`${at}.colors`, 'a theme needs all 23 color tokens'); continue; }
        const missing = TOKENS.filter((k) => !t.colors[k]);
        if (missing.length) r.error(`${at}.colors`, `missing ${missing.length} of the 23 tokens: ${missing.join(', ')}. A component that needed one of these would have rendered invisible`);
        const extra = Object.keys(t.colors).filter((k) => !TOKENS.includes(k));
        if (extra.length) r.warn(`${at}.colors`, `not a token the shell knows: ${extra.join(', ')}`);
        const rgb = {};
        for (const [k, v] of Object.entries(t.colors)) {
          const parsed = hex(String(v));
          if (!parsed) r.error(`${at}.colors.${k}`, `${JSON.stringify(v)} is not a hex color`);
          else rgb[k] = parsed;
        }
        const paper = rgb.paper;
        for (const [fg, bg] of PAIRS) {
          if (!rgb[fg] || !rgb[bg]) continue;
          const ratio = contrast(over(rgb[fg], rgb[bg], paper), over(rgb[bg], paper, paper));
          if (ratio < 4.5) {
            r.error(`${at}.colors`, `${fg} on ${bg} is ${ratio.toFixed(2)}:1, under 4.5:1. This gets read outdoors in the sun, with one hand`);
          }
        }
      }
    }
  }

  return r.done();
}

// ---------------------------------------------------------------- blocks

function checkBlocks(r, at, blocks, ctx) {
  if (!Array.isArray(blocks)) {
    r.error(at, 'has to be a list of blocks, each one a list: a time, a text, and optionally a map');
    return;
  }
  let last = '';
  blocks.forEach((b, i) => {
    const bat = `${at}[${i}]`;
    if (!Array.isArray(b)) {
      r.error(bat, 'a block is a list: ["09:00", "what happens", {type: visit}]');
      return;
    }
    if (b.length < 2) { r.error(bat, 'a block needs a time and a text'); return; }
    if (b.length > 3) { r.error(bat, `${b.length} elements. A block is a time, a text, and at most one map`); return; }
    const [time, text, meta] = b;
    const t = str(time);
    if (t !== '' && !TIME.test(t)) {
      r.error(bat, `${JSON.stringify(time)} is not a time. Quote it, "09:00", on a 24 hour clock. An empty time is a note about the whole day`);
    }
    if (typeof text !== 'string' || text.trim() === '') r.error(bat, 'the second element is the text, and it cannot be empty');
    const timed = t !== '' && TIME.test(t);
    if (timed && last && t < last) r.warn(bat, `${t} comes after ${last} in the list but earlier in the day. The reader takes the last block whose time has passed, so order matters`);
    if (timed) last = t;
    if (typeof text === 'string' && TO_CONFIRM.test(text)) r.warn(bat, 'holds something to confirm, and will render as a hole until it is confirmed');
    if (meta === undefined) return;
    if (!isMap(meta)) {
      r.error(bat, 'the third element has to be a map: {type: visit, place: azulejo}. This is the single most common mistake in a pack');
      return;
    }
    const { read, map, placeIds, personIds } = ctx;
    const type = read(meta, 'block', 'type');
    if (type !== undefined) checkType(r, `${bat}.type`, type, map);
    const place = str(read(meta, 'block', 'place'));
    if (place && placeIds.size && !placeIds.has(place)) r.error(`${bat}.place`, `${JSON.stringify(place)} is not a place in this pack`);
    else if (place && !placeIds.size) r.warn(`${bat}.place`, `this pack has no places, so ${JSON.stringify(place)} points at nothing and the screen keeps only the block's own text`);
    const forWhom = read(meta, 'block', 'for');
    if (forWhom !== undefined) {
      const list = Array.isArray(forWhom) ? forWhom : [forWhom];
      list.forEach((id) => { if (personIds.size && !personIds.has(str(id))) r.error(`${bat}.for`, `${JSON.stringify(str(id))} is not one of the people in this pack`); });
    }
    const guide = str(read(meta, 'block', 'guide'));
    if (guide && personIds.size && !personIds.has(guide)) r.error(`${bat}.guide`, `${JSON.stringify(guide)} is not one of the people in this pack`);
    const until = str(read(meta, 'block', 'until'));
    if (until && !TIME.test(until)) r.error(`${bat}.until`, `${JSON.stringify(until)} is not a time, HH:MM`);
    if (until && t && until <= t) r.error(`${bat}.until`, `${until} is not after the block's own time, ${t}`);
    const locked = read(meta, 'block', 'locked');
    if (locked !== undefined && typeof locked !== 'boolean') r.error(`${bat}.locked`, 'is true or false: an hour that cannot move, or one that can');
  });
}

function checkType(r, at, value, map) {
  const canon = canonValue(map, 'type', value);
  if (TYPES.includes(canon)) return;
  const stem = String(canon).slice(0, 3);
  const near = stem ? TYPES.find((t) => t.startsWith(stem)) : undefined;
  r.error(at, `${JSON.stringify(value)} is not one of the thirteen types${near ? `. Did you mean ${near}?` : `: ${TYPES.join(', ')}`}`);
}

// Unknown keys are the point, so this only reports the two cases worth a word.
function walkFree(r, at, obj, conventions, known, map, kind) {
  const renames = isMap(map[kind]) ? Object.values(map[kind]) : [];
  const hidden = conventions.hidden_prefixes || [];
  for (const [key, value] of Object.entries(obj)) {
    if (known.includes(key) || renames.includes(key) || key === 'id') continue;
    if (hidden.some((p) => key.startsWith(String(p)))) continue;
    const m = key.match(/__(\d{4})_(\d{2})_(\d{2})$/);
    if (m && !realDate(`${m[1]}-${m[2]}-${m[3]}`)) {
      r.error(`${at}.${key}`, 'ends in a date that does not exist, so it would never show');
    }
    if (typeof value === 'string' && TO_CONFIRM.test(value)) {
      r.warn(`${at}.${key}`, 'holds something to confirm, and renders as a hole on purpose');
    }
  }
}

// ---------------------------------------------------------------- keymap

function accessor(map) {
  return (obj, kind, canonical) => {
    if (!isMap(obj)) return undefined;
    if (obj[canonical] !== undefined) return obj[canonical];
    const renamed = isMap(map[kind]) ? map[kind][canonical] : undefined;
    return renamed ? obj[renamed] : undefined;
  };
}

function rootOf(content, map, canonical) {
  if (content[canonical] !== undefined) return content[canonical];
  const path = isMap(map.root) ? map.root[canonical] : undefined;
  if (!path) return undefined;
  let node = content;
  for (const step of String(path).split('.')) {
    if (!isMap(node)) return undefined;
    node = node[step];
  }
  return node;
}

function canonValue(map, enumName, value) {
  const table = isMap(map.values) && isMap(map.values[enumName]) ? map.values[enumName] : null;
  if (!table) return str(value);
  for (const [canonical, theirs] of Object.entries(table)) {
    if (str(theirs) === str(value)) return canonical;
  }
  return str(value);
}

// ---------------------------------------------------------------- colour

function hex(value) {
  const m = /^#([0-9a-fA-F]{6})([0-9a-fA-F]{2})?$/.exec(value.trim());
  if (!m) return null;
  const n = parseInt(m[1], 16);
  return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255, a: m[2] === undefined ? 1 : parseInt(m[2], 16) / 255 };
}

// A token with alpha is measured over what is actually behind it.
function over(color, behind, paper) {
  if (color.a === 1) return color;
  const base = behind && behind.a === 1 ? behind : (paper || { r: 255, g: 255, b: 255, a: 1 });
  return {
    r: color.r * color.a + base.r * (1 - color.a),
    g: color.g * color.a + base.g * (1 - color.a),
    b: color.b * color.a + base.b * (1 - color.a),
    a: 1,
  };
}

function luminance({ r, g, b }) {
  const f = (v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

function contrast(a, b) {
  const la = luminance(a);
  const lb = luminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

// ---------------------------------------------------------------- plumbing

class Report {
  constructor() { this.errors = []; this.warnings = []; }
  error(where, message) { this.errors.push({ where, message }); }
  warn(where, message) { this.warnings.push({ where, message }); }
  done() { return { errors: this.errors, warnings: this.warnings, ok: this.errors.length === 0 }; }
}

function isMap(v) { return v !== null && typeof v === 'object' && !Array.isArray(v); }
function str(v) { return v === undefined || v === null ? '' : String(v); }

function realDate(date) {
  const [y, m, d] = date.split('-').map(Number);
  if (!y || !m || !d || m > 12) return false;
  const days = [31, (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0 ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  return d >= 1 && d <= days[m - 1];
}

// ---------------------------------------------------------------- cli

async function main(argv) {
  const { readFileSync, statSync } = await import('node:fs');
  const { join, dirname } = await import('node:path');
  const target = argv[0];
  if (!target) {
    console.error('usage: node tools/validate.mjs <pack directory or pack.yaml>');
    return 2;
  }
  let stat;
  try {
    stat = statSync(target);
  } catch {
    console.error(`no pack at ${target}. Point me at a pack folder or at its pack.yaml`);
    return 2;
  }
  const dir = stat.isDirectory() ? target : dirname(target);
  const manifestPath = stat.isDirectory() ? join(dir, 'pack.yaml') : target;

  const readYaml = (path) => {
    try {
      return { value: parse(readFileSync(path, 'utf8')) };
    } catch (e) {
      return { error: `${path}: ${e.message}` };
    }
  };

  // A pack is a folder. A path that climbs out of it is a mistake at best, and the app will be
  // loading packs that came from somewhere else.
  const insidePack = (file) => {
    const p = String(file).replace(/\\/g, '/');
    return !p.startsWith('/') && !/^[A-Za-z]:/.test(p) && !p.split('/').includes('..');
  };
  const load = (file, where) => {
    if (!insidePack(file)) {
      return { error: `${where}: ${JSON.stringify(String(file))} leaves the pack folder. Every file a pack names lives inside it` };
    }
    return readYaml(join(dir, String(file)));
  };

  const manifest = readYaml(manifestPath);
  if (manifest.error) { console.error(manifest.error); return 1; }
  const head = (manifest.value && manifest.value.pack) || {};

  // content is one file, a list of files, or a mapping of root key to file. The mapping roots each
  // file under the key that names it, which is the only form where two files cannot collide.
  let content = {};
  if (isMap(head.content)) {
    for (const [root, file] of Object.entries(head.content)) {
      const loaded = load(file, `pack.content.${root}`);
      if (loaded.error) { console.error(loaded.error); return 1; }
      content[root] = loaded.value;
    }
  } else {
    const files = Array.isArray(head.content) ? head.content : [head.content].filter(Boolean);
    for (const file of files) {
      const loaded = load(file, 'pack.content');
      if (loaded.error) { console.error(loaded.error); return 1; }
      const value = loaded.value || {};
      for (const key of Object.keys(value)) {
        if (key in content) {
          console.error(`${file}: ${JSON.stringify(key)} is already defined in an earlier content file, and the second one would quietly win. Split the pack by key, or name the root in the manifest: content: {${key}: ${file}}`);
          return 1;
        }
      }
      content = { ...content, ...value };
    }
  }

  let theme;
  if (head.theme) {
    const loaded = load(head.theme, 'pack.theme');
    if (loaded.error) { console.error(loaded.error); return 1; }
    theme = loaded.value;
  }

  const result = validatePack({ manifest: manifest.value, content, theme });
  for (const e of result.errors) console.log(`error  ${e.where}: ${e.message}`);
  for (const w of result.warnings) console.log(`warn   ${w.where}: ${w.message}`);
  const name = head.name || head.id || target;
  if (result.ok) {
    console.log(`\n${name}: loads. ${result.warnings.length} warning${result.warnings.length === 1 ? '' : 's'}.`);
    return 0;
  }
  console.log(`\n${name}: does not load. ${result.errors.length} error${result.errors.length === 1 ? '' : 's'}, ${result.warnings.length} warning${result.warnings.length === 1 ? '' : 's'}.`);
  return 1;
}

// Run as a command, not when imported by the app.
if (typeof process !== 'undefined' && process.argv[1]) {
  const { pathToFileURL } = await import('node:url');
  if (import.meta.url === pathToFileURL(process.argv[1]).href) {
    process.exit(await main(process.argv.slice(2)));
  }
}
