#!/usr/bin/env python3
"""Extract illustrations, keeping card indices exclusively in the Rust UI."""
from pathlib import Path
from copy import deepcopy
from xml.etree import ElementTree as ET

ROOT = Path(__file__).resolve().parent.parent
SVG = 'http://www.w3.org/2000/svg'
ET.register_namespace('', SVG)
ET.register_namespace('xlink', 'http://www.w3.org/1999/xlink')
COURT = {
    'K': {'g2987', 'Layer_x0020_1-8', 'layer1-7', 'layer1-7-5'},
    'Q': {'g2987', 'Layer_x0020_1-8', 'layer1-7', 'layer1-7-4'},
    'J': {'g3039', 'Layer_x0020_1-8', 'layer1-7', 'layer1-7-6'},
}
for rank in ['A', '2', '3', '4', '5', '6', '7', '8', '9', '10', 'J', 'Q', 'K']:
    original = ET.parse(ROOT / f'assets/card-sources/{rank}S.svg').getroot()
    # Court illustrations are already enclosed by the original inner frame.
    box = '20 20 127 203' if rank in COURT else ('0 0 167.0869141 242.6669922' if rank == 'A' else '28 8 112 227')
    art = ET.Element(f'{{{SVG}}}svg', {'viewBox': box, 'preserveAspectRatio': 'xMidYMid meet'})
    pips = 0
    for child in original:
        tag = child.tag.rsplit('}', 1)[-1]
        identity = child.get('id', '')
        keep = tag == 'defs'
        if rank in COURT:
            keep |= identity in COURT[rank]
        elif rank == 'A':
            keep |= identity == 'path2987'
        else:
            is_pip = identity.startswith('layer1-7-88')
            keep |= is_pip
            pips += is_pip
        if keep:
            art.append(deepcopy(child))
    if rank.isdigit():
        assert pips == int(rank), (rank, pips)
    assert not any(el.tag == f'{{{SVG}}}text' for el in art.iter())
    for element in art.iter():
        for attribute in list(element.attrib):
            if 'inkscape' in attribute or 'sodipodi' in attribute:
                del element.attrib[attribute]
    out = ROOT / f'web/public/cards/art-v2/{rank}S.svg'
    ET.ElementTree(art).write(out, encoding='utf-8', xml_declaration=True)
print('Prepared 13 illustration-only SVGs; original indices and outer borders excluded.')

# Other suits use the same inner court frame, with distinct original portraits.
COURTS = {
 'JH': {'g2987','layer1-9-6','layer1-9-6-7'},
 'QH': {'g2987','layer1-9-6','layer1-9-6-4'},
 'KH': {'g3755','layer1-9-6','layer1-9-6-1'},
 'JC': {'g2987','layer1-1-4','layer1-1-4-8'},
 'QC': {'g2987','layer1-1-4','layer1-1-4-9'},
 'KC': {'g3823','layer1-1-4','layer1-1-4-8'},
 'JD': {'g2987','layer1-2-6','layer1-2-6-5'},
 'QD': {'g2987','layer1-2-6','layer1-2-6-5'},
 'KD': {'g3767','layer1-2-6','layer1-2-6-5'},
}
for name, ids in COURTS.items():
    original = ET.parse(ROOT / f'assets/card-sources/{name}.svg').getroot()
    art = ET.Element(f'{{{SVG}}}svg', {'viewBox':'20 20 127 203','preserveAspectRatio':'xMidYMid meet'})
    for child in original:
        if child.tag == f'{{{SVG}}}defs' or child.get('id') in ids | {'Layer_x0020_1-8'}:
            art.append(deepcopy(child))
    assert not any(el.tag == f'{{{SVG}}}text' for el in art.iter())
    for element in art.iter():
        for attribute in list(element.attrib):
            if 'inkscape' in attribute or 'sodipodi' in attribute:
                del element.attrib[attribute]
    ET.ElementTree(art).write(ROOT / f'web/public/cards/art-v2/{name}.svg', encoding='utf-8',xml_declaration=True)

# One clear central symbol on every numeric card, including aces in new suits.
paths = {
 'H': 'M0 12C-3 9-12 2-12-5C-12-13-3-15 0-8C3-15 12-13 12-5C12 2 3 9 0 12Z',
 'C': 'M0-14C-8-14-9-4-4-2C-14-6-17 7-9 9C-5 10-2 7-1 4C-1 9-3 11-5 13H5C3 11 1 9 1 4C2 7 5 10 9 9C17 7 14-6 4-2C9-4 8-14 0-14Z',
 'D': 'M0-14 11 0 0 14-11 0Z',
}
(ROOT / 'web/public/cards/numeric-S.svg').write_bytes((ROOT / 'web/public/cards/numeric-spade-v1.svg').read_bytes())
for suit, path in paths.items():
    color = '#b12e32' if suit in 'HD' else '#111710'
    svg = f'<svg xmlns="{SVG}" viewBox="-14 -16 28 32" preserveAspectRatio="xMidYMid meet"><path fill="{color}" d="{path}"/></svg>\n'
    (ROOT / f'web/public/cards/numeric-{suit}.svg').write_text(svg)
    (ROOT / f'web/public/cards/art-v2/A{suit}.svg').write_text(svg)
print('Prepared court illustrations and single-symbol numeric art for all four suits.')
