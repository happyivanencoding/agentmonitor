const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const manifest=require('../extension/manifest.json');
test('browser content scripts use Chrome-injectable .js resources',()=>{
  const files=manifest.content_scripts.flatMap(x=>x.js||[]);
  assert.ok(files.includes('extractor.js'));
  assert.ok(files.every(file=>file.endsWith('.js')));
  for(const file of files)assert.ok(fs.existsSync(path.join(__dirname,'..','extension',file)),file);
});
