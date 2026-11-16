import {readFileSync,writeFileSync} from 'node:fs';
import {createRequire} from 'node:module';
const {Resvg}=createRequire('/tmp/card-qa/package.json')('@resvg/resvg-js');
const uri=p=>'data:image/svg+xml;base64,'+readFileSync('/work/web/public/cards/'+p).toString('base64');
let body='<rect width="700" height="600" fill="#003c2a"/>';
for(const [row,suit] of ['S','H','C','D'].entries()){
 for(const [col,rank] of ['A','2','9','10','J','Q','K'].entries()){
  const x=12+col*98,y=12+row*146,color='HD'.includes(suit)?'#b12e32':'#111710';
  const face=/^\d/.test(rank)?`numeric-${suit}.svg`:`art-v2/${rank}${suit}.svg`;
  body+=`<g transform="translate(${x} ${y})"><rect width="88" height="128" rx="8" fill="#f5e7c3" stroke="#cabb94"/><text x="5" y="32" fill="${color}" font-size="30" font-weight="bold">${rank}</text><image x="69" y="10" width="13" height="19" href="${uri(`numeric-${suit}.svg`)}"/><image x="6" y="42" width="76" height="80" href="${uri(face)}"/></g>`;
 }
}
const svg=`<svg xmlns="http://www.w3.org/2000/svg" width="700" height="600" font-family="Georgia">${body}</svg>`;
writeFileSync('/out/cards-multisuit-preview.png',new Resvg(svg,{font:{fontFiles:['/fonts/Georgia.ttf'],loadSystemFonts:false}}).render().asPng());
