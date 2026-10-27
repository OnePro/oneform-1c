"""Example edit for the catalog form check (task A1): add four fields bound to the object.

  python edit_cat.py <Form.xml> [add]
Appends InputField/CheckBox elements bound to СправочникОбъект.Код, .Наименование, .ЗапретРедактирования, .Счет2020
(attributes of the test catalog) before </ChildItems>. Copy and change the names for another object.
"""
import sys
F=sys.argv[1]; mode=sys.argv[2] if len(sys.argv)>2 else "add"
s=open(F,encoding="utf-8").read()
new='''    <InputField name="НовыйКод" id="900">
      <DataPath attributeId="0">СправочникОбъект.Код</DataPath>
      <Left>8</Left>
      <Top>152</Top>
      <Width>90</Width>
      <Height>19</Height>
      <ZOrder>20</ZOrder>
      <TabOrder>21</TabOrder>
    </InputField>
    <InputField name="НовоеНаименование" id="901">
      <DataPath attributeId="0">СправочникОбъект.Наименование</DataPath>
      <Left>104</Left>
      <Top>152</Top>
      <Width>250</Width>
      <Height>19</Height>
      <ZOrder>21</ZOrder>
      <TabOrder>22</TabOrder>
    </InputField>
    <CheckBox name="НовыйЗапретРедактирования" id="902">
      <DataPath attributeId="0">СправочникОбъект.ЗапретРедактирования</DataPath>
      <Caption>
        <Item language="ru">Запрет редактирования</Item>
      </Caption>
      <Left>360</Left>
      <Top>154</Top>
      <Width>200</Width>
      <Height>15</Height>
      <ZOrder>22</ZOrder>
      <TabOrder>23</TabOrder>
    </CheckBox>
    <InputField name="НовыйСчет2020" id="903">
      <DataPath attributeId="0">СправочникОбъект.Счет2020</DataPath>
      <Left>566</Left>
      <Top>152</Top>
      <Width>230</Width>
      <Height>19</Height>
      <ZOrder>23</ZOrder>
      <TabOrder>24</TabOrder>
    </InputField>
'''
if mode=="add":
    assert s.count("  </ChildItems>\n</Form>")==1
    s=s.replace("  </ChildItems>\n</Form>",new+"  </ChildItems>\n</Form>")
open(F,"w",encoding="utf-8",newline="").write(s)
