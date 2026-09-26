// Tests for the YAML subset reader. Run them with: node --test tools/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parse, YamlError } from './yaml.mjs';

const refused = (text) => {
  try {
    parse(text);
  } catch (e) {
    return e;
  }
  return null;
};

test('maps, nested maps and a sequence of maps', () => {
  const v = parse(`people:
  - {id: rita, name: Rita, adult: true}
  - id: tomas
    name: Tomas
    adult: false
places:
  azulejo:
    name: Museu Nacional do Azulejo
    at: {lat: 38.7248, lon: -9.1139, radius_m: 120}
`);
  assert.equal(v.people.length, 2);
  assert.deepEqual(v.people[1], { id: 'tomas', name: 'Tomas', adult: false });
  assert.equal(v.places.azulejo.at.lat, 38.7248);
});

test('a block is a list of a time, a text and an optional map', () => {
  const v = parse(`blocks:
  - ["", "Nothing before 10:00."]
  - ["11:15", "The museum, top floor and come down.",
     {type: visit, place: azulejo, until: "13:00"}]
`);
  assert.deepEqual(v.blocks[0], ['', 'Nothing before 10:00.']);
  assert.equal(v.blocks[1].length, 3);
  assert.equal(v.blocks[1][2].until, '13:00');
});

test('a flow list wrapped over lines is one list', () => {
  const v = parse(`fixed: [["08:30", "Pick up the car."],
  ["20:00", "Dinner, all four."]]
`);
  assert.equal(v.fixed.length, 2);
  assert.equal(v.fixed[1][0], '20:00');
});

test('dates and times stay strings, numbers become numbers', () => {
  const v = parse(`date: 2026-04-11
time: 08:30
radius_m: 120
code: 07
price: 12.50
locked: true
`);
  assert.equal(v.date, '2026-04-11');
  assert.equal(v.time, '08:30');
  assert.equal(v.radius_m, 120);
  assert.equal(v.code, 7);   // a leading zero is a number here as it is in YAML: quote a code that needs it
  assert.equal(v.price, 12.5);
  assert.equal(v.locked, true);
});

test('a literal block, a trailing comment and an empty value', () => {
  const v = parse(`why: |
  three driving days follow
  and the car goes back on the fourth
id: coast   # the slug
detail:
severity: high
`);
  assert.match(v.why, /three driving days follow/);
  assert.equal(v.id, 'coast');
  assert.equal(v.detail, null);
  assert.equal(v.severity, 'high');
});

test('an apostrophe in a plain scalar is text, not a quote', () => {
  assert.equal(parse(`name: L'Hospitalet
`).name, "L'Hospitalet");
});

test('a file can start with a sequence, which is what a split content file holds', () => {
  const v = parse(`- date: 2026-04-11
  title: The tile museum
- date: 2026-04-12
  title: The ferry back
`);
  assert.equal(v.length, 2);
  assert.equal(v[1].title, 'The ferry back');
});

test('one leading document marker is fine, a second document is refused', () => {
  assert.equal(parse(`---
a: 1
`).a, 1);
  const e = refused(`a: 1
---
a: 2
`);
  assert.ok(e instanceof YamlError);
  assert.match(e.message, /second document/);
});

test('content after the end of the document is refused', () => {
  const e = refused(`a: 1
...
b: 2
`);
  assert.ok(e instanceof YamlError);
  assert.match(e.message, /the document ends here/);
});

test('a bad escape in a quoted scalar is a YamlError with a line, not a JSON error', () => {
  for (const text of ['k: "a\\q"\n', '"a\\q": 1\n', 'k: {"a\\q": 1}\n']) {
    const e = refused(text);
    assert.ok(e instanceof YamlError, text);
    assert.equal(e.line, 1);
  }
});

test('__proto__ as a key is refused instead of disappearing', () => {
  for (const text of ['__proto__:\n  polluted: true\n', 'k: {__proto__: {polluted: true}}\n']) {
    const e = refused(text);
    assert.ok(e instanceof YamlError, text);
    assert.match(e.message, /__proto__/);
  }
  assert.equal({}.polluted, undefined);
});
