"""Generate tiny, authored format fixtures and usable schematic examples (no game assets)."""
from pathlib import Path
import gzip
import struct

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
    examples=ROOT/'examples/schematics';examples.mkdir(parents=True,exist_ok=True)
    (examples/'demo.schem').write_bytes(gzip.compress(v3,mtime=0))
    (examples/'demo.litematic').write_bytes(gzip.compress(lite,mtime=0))
if __name__=='__main__': main()
