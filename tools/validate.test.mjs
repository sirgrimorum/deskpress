// Tests for the validator. Run them with: node --test tools/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { parse } from './yaml.mjs';
import { validatePack } from './validate.mjs';

const manifest = (over = {}) => ({
  pack: {
    id: 'test-pack', name: 'Test pack', language: 'en',
    timezone: 'Europe/Madrid', content: 'content.yaml', ...over,
  },
});
const day = (over = {}) => ({
  date: '2026-04-11', title: 'A day', blocks: [['10:00', 'Something happens.']], ...over,
});
const check = (content, theme, over) => validatePack({ manifest: manifest(over), content, theme });
const said = (result) => [...result.errors, ...result.warnings].map((e) => `${e.where}: ${e.message}`).join('\n');

// The 23 tokens, as a theme that passes every contrast pair. Override one to break it.
const FG = ['ink', 'ink-muted', 'action-ink', 'highlight-ink', 'highlight-text', 'chip-ink',
  'bar-ink', 'bar-muted', 'alert', 'missing-text'];
const BG = ['paper', 'card', 'action-bg', 'highlight-bg', 'chip-bg', 'bar-bg', 'missing-fill'];
const REST = ['card-line', 'line', 'rule', 'soft', 'highlight-line', 'missing-border'];
const palette = (over = {}) => {
  const colors = {};
  for (const k of FG) colors[k] = '#000000';
  for (const k of BG) colors[k] = '#ffffff';
  for (const k of REST) colors[k] = '#888888';
  return { themes: { plain: { name: 'Plain', mode: 'light', colors: { ...colors, ...over } } } };
};

test('the example pack loads with no errors and no warnings', () => {
  const at = (file) => fileURLToPath(new URL(`../examples/one-day/${file}`, import.meta.url));
  const result = validatePack({
    manifest: parse(readFileSync(at('pack.yaml'), 'utf8')),
    content: parse(readFileSync(at('content.yaml'), 'utf8')),
  });
  assert.equal(said(result), '');
  assert.equal(result.ok, true);
});

test('the smallest pack that says something loads', () => {
  const result = check({ days: [day()] });
  assert.equal(result.ok, true);
  assert.equal(result.warnings.length, 0);
});

test('a manifest with no pack block, and content that is not a mapping', () => {
  assert.equal(validatePack({ manifest: { nope: 1 }, content: {} }).ok, false);
  assert.match(said(validatePack({ manifest: manifest(), content: null })), /content file is missing/);
});

test('no days names the keymap instead of just failing', () => {
  assert.match(said(check({ people: [] })), /keymap\.root\.days/);
});

test('a day needs a date and a title, and two days cannot share a date', () => {
  assert.match(said(check({ days: [{ blocks: [['10:00', 'x']] }] })), /a day needs a date/);
  assert.match(said(check({ days: [{ date: '2026-04-11', blocks: [['10:00', 'x']] }] })), /a day needs a title/);
  assert.match(said(check({ days: [day(), day()] })), /appears twice/);
  assert.match(said(check({ days: [day({ date: '2026-02-30' })] })), /is not a real date/);
});

test('a block is a list, its time is quoted, and its third element is a map', () => {
  assert.match(said(check({ days: [day({ blocks: [{ time: '10:00' }] })] })), /a block is a list/);
  assert.match(said(check({ days: [day({ blocks: [['10:00', 'x', 'visit']] })] })), /has to be a map/);
  assert.match(said(check({ days: [day({ blocks: [[1000, 'x']] })] })), /is not a time/);
  assert.match(said(check({ days: [day({ blocks: [['10:00', '  ']] })] })), /cannot be empty/);
});

test('an unknown block type names the nearest one it knows', () => {
  assert.match(said(check({ days: [day({ blocks: [['10:00', 'x', { type: 'walk' }]] })] })), /Did you mean walking/);
  assert.match(said(check({ days: [day({ blocks: [['10:00', 'x', { type: 'zzz' }]] })] })), /visit, train, driving/);
});

test('a block can point at a place and a person only if the pack has them', () => {
  const content = {
    people: [{ id: 'rita', name: 'Rita', adult: true }],
    places: { azulejo: { name: 'Museu', at: { lat: 38.72, lon: -9.11, radius_m: 120 } } },
    days: [day({ blocks: [['10:00', 'x', { place: 'azulejo', guide: 'rita' }]] })],
  };
  assert.equal(check(content).ok, true);
  const wrong = JSON.parse(JSON.stringify(content));
  wrong.days[0].blocks[0][2] = { place: 'nowhere', guide: 'nobody' };
  assert.match(said(check(wrong)), /"nowhere" is not a place in this pack/);
  assert.match(said(check(wrong)), /"nobody" is not one of the people/);
});

test('a place with no coordinates loads and says why that costs something', () => {
  const result = check({ places: { hotel: { name: 'The hotel' } }, days: [day()] });
  assert.equal(result.ok, true);
  assert.match(said(result), /cannot become a geofence/);
});

test('an option day needs two options, one recommended, and a decision', () => {
  const options = [
    { id: 'coast', name: 'The coast', recommended: true, blocks: [['10:00', 'The coast road.']] },
    { id: 'hill', name: 'The hill', blocks: [['10:00', 'The hill road.']] },
  ];
  const decision = { when: '2026-04-01', at: '21:00' };
  const ok = check({ days: [{ date: '2026-04-11', title: 'Two plans', options, decision }] });
  assert.equal(said(ok), '');

  const one = [options[0]];
  assert.match(said(check({ days: [{ date: '2026-04-11', title: 'x', options: one, decision }] })), /one option is not an option/);
  assert.match(said(check({ days: [{ date: '2026-04-11', title: 'x', options }] })), /needs a decision block/);

  const both = [options[0], { ...options[1], recommended: true }];
  assert.match(said(check({ days: [{ date: '2026-04-11', title: 'x', options: both, decision }] })), /Only one can be/);

  const neither = [{ ...options[0], recommended: false }, options[1]];
  assert.match(said(check({ days: [{ date: '2026-04-11', title: 'x', options: neither, decision }] })), /nothing to behave as/);

  const late = { when: '2026-04-12', at: '21:00' };
  assert.match(said(check({ days: [{ date: '2026-04-11', title: 'x', options, decision: late }] })), /before it stops being a question/);
});

test('decides and requires have to name a day and an option that exist', () => {
  const options = [
    { id: 'coast', name: 'The coast', recommended: true, blocks: [['10:00', 'a']] },
    { id: 'hill', name: 'The hill', blocks: [['10:00', 'b']] },
  ];
  const fork = { date: '2026-04-11', title: 'Two plans', options, decision: { when: '2026-04-01', decides: ['2026-04-30'] } };
  assert.match(said(check({ days: [fork] })), /"2026-04-30" is not a day in this pack/);


  const dependent = {
    date: '2026-04-12', title: 'The second fork',
    options: [
      { id: 'near', name: 'Near', recommended: true, requires: { date: '2026-04-11', option: 'nope' }, blocks: [['11:00', 'c']] },
      { id: 'far', name: 'Far', requires: { date: '2026-04-11', option: 'coast' }, blocks: [['11:00', 'd']] },
    ],
    decision: { when: '2026-04-10' },
  };
  const fine = { ...fork, decision: { when: '2026-04-01' } };
  assert.match(said(check({ days: [fine, dependent] })), /"nope" is not an option on 2026-04-11\. It has coast, hill/);
});

test('an alert needs a title and a severity the shell knows', () => {
  const alert = { id: 'ferry', title: 'The last ferry', severity: 'high', at: '2026-04-11T18:40' };
  assert.equal(check({ days: [day()], alerts: [alert] }).ok, true);
  assert.match(said(check({ days: [day()], alerts: [{ ...alert, severity: 'urgent' }] })), /is not a severity/);
  assert.match(said(check({ days: [day()], alerts: [{ ...alert, title: undefined }] })), /an alert needs a title/);
  assert.match(said(check({ days: [day()], alerts: [alert, alert] })), /"ferry" is used twice/);
});

test('a document needs a title, a file, and an id nothing else uses', () => {
  const doc = { id: 'passport_rita', title: 'Passport, Rita', file: 'files/passport.pdf' };
  const people = [{ id: 'rita', name: 'Rita', adult: true }];
  assert.equal(check({ days: [day()], people, documents: [{ ...doc, for: 'rita' }] }).ok, true);
  assert.match(said(check({ days: [day()], documents: [doc, doc] })), /"passport_rita" is used twice/);
  assert.match(said(check({ days: [day()], documents: [{ ...doc, file: undefined }] })), /needs a file/);
  assert.match(said(check({ days: [day()], people, documents: [{ ...doc, for: 'nobody' }] })), /is not one of the people/);
});

test('a keymap moves the root and the keys inside a day', () => {
  const content = { itinerario: { dias: [{ fecha: '2026-04-11', titulo: 'Un dia', bloques: [['10:00', 'Algo pasa.']] }] } };
  const keymap = { root: { days: 'itinerario.dias' }, day: { date: 'fecha', title: 'titulo', blocks: 'bloques' } };
  assert.equal(said(validatePack({ manifest: { ...manifest(), keymap }, content })), '');
});

test('a severity written in another language passes once it is mapped', () => {
  const alerts = [{ id: 'ferry', title: 'El ultimo ferry', severity: 'alta' }];
  const keymap = { values: { severity: { high: 'alta' } } };
  assert.equal(validatePack({ manifest: { ...manifest(), keymap }, content: { days: [day()], alerts } }).ok, true);
  assert.match(said(check({ days: [day()], alerts })), /is not a severity/);
});

test('the 23 tokens are all required, and every pair is read at 4.5:1', () => {
  assert.equal(check({ days: [day()] }, palette()).ok, true);
  const short = palette();
  delete short.themes.plain.colors.ink;
  assert.match(said(check({ days: [day()] }, short)), /missing 1 of the 23 tokens: ink/);
  assert.match(said(check({ days: [day()] }, palette({ ink: '#dddddd' }))), /under 4\.5:1/);
  assert.match(said(check({ days: [day()] }, palette({ ink: 'black' }))), /is not a hex color/);
  const noMode = palette();
  delete noMode.themes.plain.mode;
  assert.match(said(check({ days: [day()] }, noMode)), /has to be light or dark/);
});

test('an unknown key is the feature, and one to confirm is a warning', () => {
  assert.equal(check({ days: [day({ what_we_learned: 'The market closes at two.' })] }).ok, true);
  assert.match(said(check({ days: [day({ note: 'price is [to confirm]' })] })), /renders as a hole/);
  assert.match(said(check({ days: [day({ closed__2026_02_30: 'never' })] })), /ends in a date that does not exist/);
});

// The command itself: the three things main() decides that validatePack never sees.
test('the command on a folder: the three content forms, and the two refusals', () => {
  const cli = fileURLToPath(new URL('./validate.mjs', import.meta.url));
  const run = (dir) => {
    const res = spawnSync(process.execPath, [cli, dir], { encoding: 'utf8' });
    return { code: res.status, out: `${res.stdout}${res.stderr}` };
  };
  const made = [];
  const pack = (name, content, files) => {
    const dir = mkdtempSync(join(tmpdir(), 'deskpress-'));
    made.push(dir);
    writeFileSync(join(dir, 'pack.yaml'),
      `pack:\n  id: ${name}\n  name: ${name}\n  language: en\n  timezone: Europe/Madrid\n  content: ${content}\n`);
    for (const [file, text] of Object.entries(files)) writeFileSync(join(dir, file), text);
    return dir;
  };
  const oneDay = (date, title) => `- date: ${date}\n  title: ${title}\n  blocks: [["10:00", "Something."]]\n`;

  const one = run(pack('one', 'content.yaml', { 'content.yaml': `days:\n${oneDay('2026-04-11', 'A day')}` }));
  assert.equal(one.code, 0, one.out);

  // A map roots each file under the key that names it, so two files cannot collide.
  const split = run(pack('split', '\n    days: days.yaml\n    places: places.yaml', {
    'days.yaml': oneDay('2026-04-11', 'A day'),
    'places.yaml': 'museum:\n  name: The museum\n  at: {lat: 41.38, lon: 2.17, radius_m: 120}\n',
  }));
  assert.equal(split.code, 0, split.out);

  // A list merges in order, and two files defining the same root used to drop the first one.
  const clash = run(pack('clash', '[a.yaml, b.yaml]', {
    'a.yaml': `days:\n${oneDay('2026-04-11', 'First')}`,
    'b.yaml': `days:\n${oneDay('2026-04-12', 'Second')}`,
  }));
  assert.equal(clash.code, 1);
  assert.match(clash.out, /"days" is already defined in an earlier content file/);
  assert.match(clash.out, /content: \{days: b\.yaml\}/);

  const escape = run(pack('escape', '../elsewhere.yaml', {}));
  assert.equal(escape.code, 1);
  assert.match(escape.out, /leaves the pack folder/);

  const missing = run(join(tmpdir(), 'deskpress-no-pack-here'));
  assert.equal(missing.code, 2);
  assert.match(missing.out, /no pack at /);

  for (const dir of made) rmSync(dir, { recursive: true, force: true });
});
