import re,sys
F="src/Forms/ФормаВыбораПоиска/Form.xml"
ro = sys.argv[1]=="ro"
s=open(F,encoding="utf-8",newline="").read()
nl="\r\n" if "\r\n" in s else "\n"
main=re.search(r'<MainAttribute attributeId="(\d+)"',s).group(1)
def col(i,name,w,extra=""):
    return f'''        <Column name="{name}" id="{i}">
          <DataPath attributeId="{main}">ОбработкаОбъект.Корзина.{name}</DataPath>
          <Header><Item language="ru">{name}</Item></Header>
          <Width>{w}</Width>
{extra}        </Column>
'''
ed="          <ReadOnly>true</ReadOnly>\n" if ro else "          <Editor>InputField</Editor>\n"
cols=col(0,"НомерСтроки",30,"          <ReadOnly>true</ReadOnly>\n")+col(1,"Номенклатура",150,ed)+col(2,"Количество",80,"          <Editor>InputField</Editor>\n")
blk=f'''    <Table name="АгентТаблица" id="990">
      <DataPath attributeId="{main}">ОбработкаОбъект.Корзина</DataPath>
      <Left>8</Left>
      <Top>200</Top>
      <Width>350</Width>
      <Height>80</Height>
      <ZOrder>90</ZOrder>
      <TabOrder>90</TabOrder>
      <Columns>
{cols}      </Columns>
    </Table>
'''.replace("\n",nl)
e="  </ChildItems>"
i=s.rindex(e)
open(F,"w",encoding="utf-8",newline="").write(s[:i]+blk+s[i:])
