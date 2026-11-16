import { readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
const { Resvg } = createRequire('/tmp/card-qa/package.json')('@resvg/resvg-js');
const root = '/work/web/public/cards/art-v2/';
function card(rank,x,y,width,unit) {
  const height=128*unit;
  const image=readFileSync(/^\d+$/.test(rank) ? '/work/web/public/cards/numeric-spade-v1.svg' : root+rank+'S.svg').toString('base64');
  return `<g transform="translate(${x} ${y})">
    <rect width="${width}" height="${height}" rx="${8*unit}" fill="#f5e7c3" stroke="#d5c49b" stroke-width=".7"/>
    <text x="${width*.05}" y="${20*unit}" dominant-baseline="central" font-size="${(rank==='10'?33:36)*unit}" font-weight="bold">${rank}</text>
    <path transform="translate(${width*.86} ${20*unit}) scale(${.85*unit})" d="M0-13C-5-7-11-4-11 2C-11 7-4 9-1 3C-1 8-3 10-5 12H5C3 10 1 8 1 3C4 9 11 7 11 2C11-4 5-7 0-13Z"/>
    <image x="${width*.06}" y="${42*unit}" width="${width*.88}" height="${80*unit}" preserveAspectRatio="xMidYMid meet" href="data:image/svg+xml;base64,${image}"/>
  </g>`;
}
const columns=[['7','6','5','4','3'],['8','8','4','3','2'],['7','K'],['A','6','A','5'],['9','8','7','6','5','4'],['K','Q','J','7'],['K','Q','3'],['A','10'],['A','10'],['K','6','A','J','10']];
let content=`<rect width="390" height="440" fill="#003c2a"/><text x="14" y="23" fill="#f4e6bd" font-size="14">Одна крупная пика на картах 2–10</text>`;
['2','7','9','10'].forEach((rank,i)=>content+=card(rank,14+i*94,40,80,1));
content+='<text x="14" y="232" fill="#f4e6bd" font-size="13">Те же карты в столбцах на ширине 390 px</text>';
const unit=.4212, width=33.1;
columns.forEach((ranks,col)=>ranks.forEach((rank,row)=>content+=card(rank,7+col*(width+5),250+row*42*unit,width,unit)));
const svg=`<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="390" height="440" viewBox="0 0 390 440" font-family="Georgia" fill="#111710">${content}</svg>`;
const png=new Resvg(svg,{fitTo:{mode:'width',value:780},font:{fontFiles:['/fonts/Georgia.ttf'],loadSystemFonts:false,defaultFontFamily:'Georgia'}}).render().asPng();
writeFileSync('/out/cards-single-spade-preview.png',png);
console.log('Rendered illustration layout preview: 780 x 880');
