// Exercise only our generated catalogue page. Do not load archived third-party pages.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {JSDOM, VirtualConsole} = require('../work/browser-test/node_modules/jsdom');
const root = path.resolve(__dirname,'..');
const html=fs.readFileSync(path.join(root,'catalog/index.html'),'utf8');
const errors=[];const console=new VirtualConsole();console.on('jsdomError',e=>errors.push(e));
const dom=new JSDOM(html,{runScripts:'dangerously',url:'file://'+root+'/catalog/index.html',virtualConsole:console});
const w=dom.window,d=w.document,$=s=>d.querySelector(s);
function input(id,value){$(id).value=value;$(id).dispatchEvent(new w.Event('input'));}
function tab(name){$(`[data-group="${name}"]`).click();}
assert.equal(errors.length,0);
assert.equal($('#body').children.length,100);
$('#more').click();assert.equal($('#body').children.length,200);
input('#query','Antminer S9');assert.ok($('#body').children.length>0);
$('#body button').click();assert.equal($('#detail').style.display,'block');
$('#close').click();assert.equal($('#detail').style.display,'none');
input('#query','');tab('artifacts');input('#status','failed');
assert.ok($('#body').children.length>0);
for(const row of $('#body').children)assert.equal(row.children[2].textContent,'failed');
input('#status','');input('#query','tool-qemu');
const qemu=Array.from($('#body').children).find(r=>r.children[0].textContent.includes('qemu'));
assert.ok(qemu);qemu.querySelector('button').click();
let record=JSON.parse($('#record').textContent);assert.ok(record.local_path);
// A source and its artifact can have the same ID. The source link must retain its type.
const sourceButton=Array.from($('#refs button')).find(b=>b.textContent.includes('source repository'));
if(sourceButton){sourceButton.click();record=JSON.parse($('#record').textContent);assert.ok(record.artifact_ids);$('#refs button').click();record=JSON.parse($('#record').textContent);assert.ok(record.local_path);}
for(const a of d.querySelectorAll('a[href]'))assert.ok(!/^javascript:/i.test(a.href));
w.location.hash='#artifacts/tool-qemu';w.dispatchEvent(new w.HashChangeEvent('hashchange'));
record=JSON.parse($('#record').textContent);assert.equal(record.id,'tool-qemu');assert.ok(record.local_path);
assert.ok(Array.from(d.querySelectorAll('#refs a')).some(a=>a.textContent==='Link to this record'&&a.hash==='#artifacts/tool-qemu'));
w.location.hash='#artifacts/%ZZ';assert.doesNotThrow(()=>w.dispatchEvent(new w.HashChangeEvent('hashchange')));
assert.equal(errors.length,0);dom.window.close();process.stdout.write('Browser tests passed: pagination, search, details, status filters, local links.\n');
