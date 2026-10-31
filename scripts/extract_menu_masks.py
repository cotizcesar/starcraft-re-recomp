from PIL import Image
import numpy as np
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
refs={rid:np.asarray(Image.open(ROOT/f'engine-rust/docs/original_screenshots/Screenshot_20261027_{rid}.png').convert('RGB').crop((440,0,3000,1440))).astype(np.float32) for rid in ['130637','130704','130712']}
bgs={k:np.asarray(Image.open(f'/tmp/opencode/menu-shots/bg-{k}.png').convert('RGB')).astype(np.float32) for k in ['idle','single','multi']}
# Requires the no-text renders bg-{idle,single,multi}.png (GC_NO_TEXT=1) in /tmp/opencode/menu-shots.
INFO=np.array([198,218,255],np.float32); BRIGHT=np.array([208,255,131],np.float32); NORMAL=np.array([102,227,53],np.float32); HOVER=np.array([196,255,124],np.float32)
def solve(R,B,F):
    D=F-B; den=(D*D).sum(axis=2); num=((R-B)*D).sum(axis=2)
    a=np.where(den>1e-3,num/np.maximum(den,1e-3),0).clip(0,1)
    out=B+a[:,:,None]*(F-B)
    return a,float(np.abs(R-out).mean())
# name,rid,bg,bbox,F0,F1,split
items=[
 ('profile','130637','idle',(1980,0,2230,40),INFO,INFO,0),
 ('version','130637','idle',(1780,1395,2230,1435),INFO,INFO,0),
 ('single','130637','idle',(760,340,1140,400),BRIGHT,NORMAL,808),
 ('multi','130637','idle',(540,580,880,635),BRIGHT,NORMAL,608),
 ('options','130637','idle',(430,1035,690,1090),BRIGHT,NORMAL,472),
 ('collections','130637','idle',(390,1115,730,1160),BRIGHT,NORMAL,435),
 ('hotkeys','130637','idle',(430,1185,700,1240),BRIGHT,NORMAL,468),
 ('credits','130637','idle',(350,1265,770,1310),BRIGHT,NORMAL,397),
 ('intro','130637','idle',(400,1340,720,1385),BRIGHT,NORMAL,440),
 ('exit','130637','idle',(1780,970,1920,1015),BRIGHT,NORMAL,1820),
 ('single_hover','130704','single',(760,340,1160,410),HOVER,HOVER,0),
 ('multi_hover','130712','multi',(500,575,950,640),HOVER,HOVER,0),
]
for name,rid,bg,box,F0,F1,split in items:
    x0,y0,x1,y1=box
    R=refs[rid][y0:y1,x0:x1]; B=bgs[bg][y0:y1,x0:x1]
    if split:
        xs=np.arange(x0,x1)[None,:,None]
        F=np.where(xs<split,F0[None,None,:],F1[None,None,:]).astype(np.float32)
        # constant per pixel; solve vectorized using per-pixel F
        D=F-B; den=(D*D).sum(axis=2); num=((R-B)*D).sum(axis=2)
        a=np.where(den>1e-3,num/np.maximum(den,1e-3),0).clip(0,1)
        out=B+a[:,:,None]*(F-B)
    else:
        a,err=solve(R,B,F0); out=B+a[:,:,None]*(F0-B)
    err=float(np.abs(R-out).mean())
    print(name,'orig',round(float(np.abs(R-B).mean()),2),'mask',round(err,2),'alpha>0.05',int((a>0.05).sum()))
