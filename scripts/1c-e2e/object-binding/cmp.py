import sys,re,collections
def load(p,ph):
    d={}
    for l in open(p,encoding='utf-8-sig').read().splitlines():
        if l.startswith(ph+':') and '=' in l:
            k,v=l.split('=',1); d[k[len(ph)+1:]]=v
    return d
PH=sys.argv[4] if len(sys.argv)>4 else 'post'
a,b=load(sys.argv[1],PH),load(sys.argv[2],PH)
new=set(sys.argv[3].split(',')) if len(sys.argv)>3 else set()
def ctl(k): return k.split('.')[0]
ca={ctl(k) for k in a}; cb={ctl(k) for k in b}
print('props before',len(a),'after',len(b),'controls before',len(ca),'after',len(cb))
print('controls added:',sorted(cb-ca),' removed:',sorted(ca-cb))
miss=[k for k in a if k not in b]; diff=[(k,a[k],b[k]) for k in a if k in b and a[k]!=b[k]]
extra=[k for k in b if k not in a and ctl(k) not in (cb-ca)]
print('missing in after:',len(miss),miss[:5]); print('different values:',len(diff))
for d in diff[:20]: print('  ',d)
print('extra props on pre-existing controls:',len(extra),extra[:5])
print('props on new controls:',sum(1 for k in b if ctl(k) in (cb-ca)))
