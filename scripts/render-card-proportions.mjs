import {readFileSync,writeFileSync} from 'node:fs';
import {createRequire} from 'node:module';
const {Resvg}=createRequire('/tmp/card-qa/package.json')('@resvg/resvg-js');
const root='/work/web/public/cards/';
const uri=p=>'data:image/svg+xml;base64,'+readFileSync(root+p).toString('base64');
const cols=[['7S','6S','5S','4S','3S'],['8H','7H','6H'],['KS','QS','JS'],['9D','8D'],['10C','9C','8C'],['KH','QH'],['JD','10D'],['AC','10H'],['6S','5D'],['KD','QC','JC']];
let content='<rect width="810" height="270" fill="#003c2a"/>';
for(const [panel,height] of [160,128].entries()){
 const origin=10+panel*400,unit=.4212,width=33.1;
 content+=`<text x="${origin}" y="22" fill="#f4e6bd" font-size="14">${panel?'После: высота −20%':'До'} · ширина экрана 390 px</text>`;
 for(const [col,cards] of cols.entries())for(const [row,name] of cards.entries()){
  const rank=name.slice(0,-1),suit=name.at(-1),color='HD'.includes(suit)?'#b12e32':'#111710';
  const face=/^\d/.test(rank)?`numeric-${suit}.svg`:`art-v2/${rank}${suit}.svg`;
  content+=`<g transform="translate(${origin+7+col*(width+5)} ${42+row*42*unit})"><rect width="${width}" height="${height*unit}" rx="${8*unit}" fill="#f5e7c3" stroke="#cabb94" stroke-width=".7"/><text x="${width*.05}" y="${29*unit}" font-size="${(rank==='10'?33:36)*unit}" fill="${color}" font-weight="bold">${rank}</text><image x="${width*.8}" y="${10*unit}" width="${width*.15}" height="${20*unit}" href="${uri(`numeric-${suit}.svg`)}"/><image x="${width*.06}" y="${(panel?42:44)*unit}" width="${width*.88}" height="${(panel?80:108)*unit}" href="${uri(face)}"/></g>`;
 }
}
const svg=`<svg xmlns="http://www.w3.org/2000/svg" width="810" height="270" font-family="Georgia">${content}</svg>`;
writeFileSync('/out/cards-proportions-preview.png',new Resvg(svg,{fitTo:{mode:'width',value:1620},font:{fontFiles:['/fonts/Georgia.ttf'],loadSystemFonts:false}}).render().asPng());
