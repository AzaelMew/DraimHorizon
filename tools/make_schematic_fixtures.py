"""Generate tiny, authored format fixtures and usable schematic examples (no game assets)."""
from pathlib import Path
import gzip
import struct
import copy

ROOT = Path(__file__).resolve().parents[1]
def text(v):
    b = v.encode('utf-16-be')
    out = bytearray()
    for i in range(0, len(b), 2):
        c = int.from_bytes(b[i:i+2], 'big')
        out.extend(bytes([c]) if 0 < c < 128 else bytes([192 | c >> 6, 128 | c & 63]) if c < 2048 else bytes([224 | c >> 12, 128 | c >> 6 & 63, 128 | c & 63]))
    return struct.pack('>H', len(out)) + out
def payload(t, v):
    if t == 2: return struct.pack('>h', v)
    if t == 3: return struct.pack('>i', v)
    if t == 6: return struct.pack('>d', v)
    if t == 8: return text(v)
    if t == 7: return struct.pack('>i', len(v)) + bytes(v)
    if t == 11: return struct.pack('>i', len(v)) + b''.join(struct.pack('>i', x) for x in v)
    if t == 12: return struct.pack('>i', len(v)) + b''.join(struct.pack('>Q', x & ((1 << 64)-1)) for x in v)
    if t == 9:
        ty, values = v
        return bytes([ty]) + struct.pack('>i', len(values)) + b''.join(payload(ty, x) for x in values)
    if t == 10: return b''.join(bytes([ty]) + text(k) + payload(ty, x) for k, (ty, x) in v.items()) + b'\0'
    raise ValueError(t)
def nbt(v): return b'\x0a\0\0' + payload(10, v)
def vec(x, y, z): return {k:(3,v) for k,v in zip(('x','y','z'),(x,y,z))}
def state(name, **properties):
    v = {'Name':(8,name)}
    if properties: v['Properties']=(10,{k:(8,x) for k,x in properties.items()})
    return v
def packed(values, bits):
    words=[0]*((len(values)*bits+63)//64)
    for i,v in enumerate(values):
        bit=i*bits; words[bit//64] |= v << bit%64
        if bit%64+bits>64: words[bit//64+1] |= v >> (64-bit%64)
    return [x & ((1<<64)-1) for x in words]
def main():
    folder=ROOT/'tests/fixtures/schematics'; folder.mkdir(parents=True,exist_ok=True)
    palette={'minecraft:air':(3,0),'minecraft:cobblestone':(3,1),'minecraft:oak_log[axis=y]':(3,2)}
    ids=[1]*9+[0]*9;ids[13]=2
    common={'Version':(3,2),'DataVersion':(3,3955),'Width':(2,3),'Height':(2,2),'Length':(2,3),'Offset':(11,[-1,0,-1])}
    v2=nbt(dict(common,Palette=(10,palette),BlockData=(7,ids)))
    common['Version']=(3,3)
    v3=nbt({'Schematic':(10,dict(common,Blocks=(10,{'Palette':(10,palette),'Data':(7,ids)})))})
    region={'Size':(10,vec(-3,2,3)),'Position':(10,vec(1,0,-1)),'BlockStatePalette':(9,(10,[state('minecraft:air'),state('minecraft:cobblestone'),state('minecraft:oak_log',axis='y')])),'BlockStates':(12,packed(ids,2))}
    lite=nbt({'Version':(3,7),'MinecraftDataVersion':(3,3955),'Regions':(10,{'Test 🌱':(10,region)})})
    cross={'Size':(10,vec(32,1,1)),'Position':(10,vec(0,0,0)),'BlockStatePalette':(9,(10,[state('minecraft:air')]+[state('minecraft:stone',test=str(i)) for i in range(4)])),'BlockStates':(12,packed([i%5 for i in range(32)],3))}
    cross=nbt({'Version':(3,7),'Regions':(10,{'cross':(10,cross)})})
    # A build past the earlier limits (128 to a side, 32 768 blocks): 140 x 4 x 140, the two lower
    # layers filled (39 200 blocks), stone and a log column at one corner.
    size=(140,4,140); n=size[0]*size[1]*size[2]
    ids=[0]*n
    for i in range(n):
        x=i%size[0]; z=(i//size[0])%size[2]; y=i//(size[0]*size[2])
        if y<2: ids[i]=2 if (x,z)==(0,0) else 1
    large={'Size':(10,vec(*size)),'Position':(10,vec(0,0,0)),'BlockStatePalette':(9,(10,[state('minecraft:air'),state('minecraft:stone'),state('minecraft:oak_log',axis='y')])),'BlockStates':(12,packed(ids,2))}
    large=nbt({'Version':(3,7),'MinecraftDataVersion':(3,4189),'Regions':(10,{'large':(10,large)})})
    for name,data in [('sponge-v2.nbt',v2),('sponge-v3.nbt',v3),('litematic-v7-negative.nbt',lite),('litematic-cross-word.nbt',cross),('litematic-large.nbt',large)]: (folder/name).write_bytes(data)
    # Java structure-block files: sparse positions, a state palette, and metadata that
    # imports deliberately omit. Every fixture is authored here, without game resources.
    coords = lambda values: (9, (3, list(values)))
    states = [state('minecraft:air'), state('minecraft:cobblestone'),
              state('minecraft:oak_log', axis='y'), state('minecraft:structure_void'),
              state('minecraft:chest', facing='north', type='single', waterlogged='false')]
    blocks = [{'pos':coords((x, 0, z)), 'state':(3, 1)} for z in range(3) for x in range(3)]
    blocks[0]['state'] = (3, 4)
    blocks[0]['nbt'] = (10, {'Items':(9, (10, [])), 'CustomName':(8, 'Authored fixture')})
    blocks += [{'pos':coords((1, 1, 1)), 'state':(3, 2)},
               {'pos':coords((0, 1, 0)), 'state':(3, 0)},
               {'pos':coords((2, 1, 2)), 'state':(3, 3)}]
    entities = [{'pos':(9, (6, [1.5, 0.0, 1.5])), 'blockPos':coords((1, 0, 1)),
                 'nbt':(10, {'id':(8, 'minecraft:pig')})}]
    structure = {'DataVersion':(3, 3955), 'size':coords((3, 2, 3)),
                 'palette':(9, (10, states)), 'blocks':(9, (10, blocks)),
                 'entities':(9, (10, entities))}
    (folder/'structure.nbt').write_bytes(nbt(structure))
    variants = copy.deepcopy(structure)
    variants.pop('palette')
    alternative = copy.deepcopy(states)
    alternative[1] = state('minecraft:stone')
    variants['palettes'] = (9, (9, [(10, states), (10, alternative)]))
    (folder/'structure-palettes.nbt').write_bytes(nbt(variants))
    sparse = {'size':coords((256, 64, 256)), 'palette':(9, (10, [states[2]])),
              'blocks':(9, (10, [{'pos':coords((255, 63, 255)), 'state':(3, 0)}]))}
    (folder/'structure-sparse.nbt').write_bytes(nbt(sparse))
    invalid = {}
    for name, value in [('negative-size', (-3, 2, 3)), ('volume', (256, 256, 256))]:
        case = copy.deepcopy(structure); case['size'] = coords(value); invalid[name] = case
    for name, value in [('bad-index', len(states)), ('negative-index', -1)]:
        case = copy.deepcopy(structure); case['blocks'][1][1][0]['state'] = (3, value); invalid[name] = case
    for name, value in [('outside', (3, 0, 0)), ('negative-pos', (-1, 0, 0)), ('short-pos', (0, 0)), ('duplicate', (1, 0, 0))]:
        case = copy.deepcopy(structure); case['blocks'][1][1][0]['pos'] = coords(value); invalid[name] = case
    case = copy.deepcopy(structure); case['palette'] = (9, (10, [])); invalid['empty-palette'] = case
    case = copy.deepcopy(variants); case['palettes'] = (9, (9, [(10, states), (10, alternative[:-1])]))
    invalid['palette-mismatch'] = case
    for name, case in invalid.items(): (folder/f'structure-invalid-{name}.nbt').write_bytes(nbt(case))
    examples=ROOT/'examples/schematics';examples.mkdir(parents=True,exist_ok=True)
    (examples/'demo.schem').write_bytes(gzip.compress(v3,mtime=0))
    (examples/'demo.litematic').write_bytes(gzip.compress(lite,mtime=0))
    (examples/'demo.nbt').write_bytes(gzip.compress(nbt(structure),mtime=0))
if __name__=='__main__': main()
