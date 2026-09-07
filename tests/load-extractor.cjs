const fs=require('node:fs');
const path=require('node:path');
const source=fs.readFileSync(path.join(__dirname,'..','extension','extractor.js'),'utf8');
const moduleBox={exports:{}};
new Function('module','URL',source)(moduleBox,URL);
module.exports=moduleBox.exports;
