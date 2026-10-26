import re,sys
DIR=sys.argv[1] if len(sys.argv)>1 else "target/e2e"
ONLY=set(sys.argv[2].split(",")) if len(sys.argv)>2 else None
p=DIR+"/src/Forms/ОсновнаяФорма/Form.xml"
on=lambda k: ONLY is None or k in ONLY
t=open(p,encoding="utf-8",newline="").read().replace("\r\n","\n")
L=t.split("\n")
def block(name):
    i=next(k for k,l in enumerate(L) if f'name="{name}" id=' in l and re.match(r"^\s+<[A-Z]",l))
    ind=re.match(r"\s*",L[i]).group(0); tag=re.search(r"<(\w+)",L[i]).group(1)
    j=next(k for k in range(i,len(L)) if L[k]==f"{ind}</{tag}>")
    return i,j,ind
def sub(name,old,new,count=1):
    i,j,_=block(name); seg="\n".join(L[i:j+1])
    assert old in seg,(name,old)
    seg=seg.replace(old,new,count); L[i:j+1]=seg.split("\n")
# E1
if on("E1"):
  sub("Надпись5","<Item language=\"ru\">Контрагент:</Item>","<Item language=\"ru\">Контрагент (тест):</Item>")
  sub("Надпись5","<Left>34</Left>","<Left>44</Left>")
# E2 font size
if on("E2"):
  sub("НадписьПоиск",'<Font bold="true" source="bin"/>','<Font bold="true" source="bin" size="12"/>')
# E3 text color for checkbox
if on("E3"):
  sub("ЗапрашиватьЦену",'<Font bold="true" source="bin"/>','<Font bold="true" source="bin"/>\n          <TextColor kind="rgb" value="#FF0000"/>')
# E4 input field
if on("E4"):
  sub("Контрагент","<Width>121</Width>","<Width>150</Width>")
  sub("Контрагент","<ClearButton>true</ClearButton>","<ClearButton>false</ClearButton>")
# E6 remove checkbox
if on("E6"):
  i,j,_=block("ПодбиратьНоменклатуруВОсновномОкне"); del L[i:j+1]
# E5 add label after Надпись5 inside the same page: siblings = same indent
if on("E5") or on("E5b"):
  i,j,ind=block("Надпись5")
  # find enclosing page end: next line equal to ind[:-2]+"</ChildItems>"
  end=next(k for k in range(j,len(L)) if L[k]==ind[:-2]+"</ChildItems>")
  zs=[int(m.group(1)) for k in range(i,end) for m in [re.match(r"^"+ind+r"  <ZOrder>(\d+)<",L[k])] if m]
  ts=[int(x) for x in re.findall(r"<TabOrder>(\d+)<","\n".join(L))]
  ids=[int(x) for x in re.findall(r' id="(\d+)"',"\n".join(L))]
  new=[f'{ind}<LabelDecoration name="АгентНовая" id="{max(ids)+1}">',
       f'{ind}  <Caption><Item language="ru">Добавлено агентом</Item></Caption>',
       f'{ind}  <Left>300</Left><Top>336</Top><Width>150</Width><Height>22</Height>',
       f'{ind}  <ZOrder>{max(zs)+1}</ZOrder><TabOrder>{max(ts)+1}</TabOrder>',
         *([f'{ind}  <Anchors top="Top" topOffset="336" left="Left" leftOffset="300"/>'] if on("E5b") else []),
       f'{ind}</LabelDecoration>']
  L[end:end]=new
open(p,"w",encoding="utf-8",newline="").write("\n".join(L))
print("edited; new z/tab",max(zs)+1,max(ts)+1)
