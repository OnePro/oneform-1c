import sys
p='src/Forms/Форма/Form.xml'
s=open(p,encoding='utf-8',newline='').read()
bar='''    <CommandBar name="ОсновныеДействияФормы" id="900">
      <Left>0</Left>
      <Top>585</Top>
      <Width>626</Width>
      <Height>25</Height>
      <ZOrder>19</ZOrder>
      <TabOrder>20</TabOrder>
      <Anchors top="Bottom" topTarget="900" topOffset="-25" bottom="Bottom" left="Left" right="Right"/>
      <AutoFill>false</AutoFill>
      <HorizontalAlign>Right</HorizontalAlign>
      <Buttons>
        <Button name="Закрыть" kind="Action" actionKind="Standard" action="Standard:142"/>
        <Button name="Разд1" kind="Separator"/>
        <Button name="ПроверитьОбмен" kind="Action" action="ОсновныеДействияФормыПроверитьОбмен">
          <Caption><Item language="ru">Проверить обмен</Item></Caption>
        </Button>
        <Button name="Сервис" kind="Submenu">
          <Caption><Item language="ru">Сервис</Item></Caption>
          <Buttons>
            <Button name="Очистить" kind="Action" action="ОсновныеДействияФормыОчистить">
              <Caption><Item language="ru">Очистить</Item></Caption>
            </Button>
          </Buttons>
        </Button>
      </Buttons>
    </CommandBar>
'''
k='  </ChildItems>\n</Form>'
i=s.rindex(k)
s=s[:i]+bar+s[i:]
open(p,'w',encoding='utf-8',newline='').write(s)
m='src/Forms/Форма/FormModule.bsl'
t=open(m,encoding='utf-8',newline='').read()
add='''Процедура ОсновныеДействияФормыПроверитьОбмен(Кнопка)

	Сообщить("Проверка обмена: " + РежимОбмена);

КонецПроцедуры

Процедура ОсновныеДействияФормыОчистить(Кнопка)

	ИмяФайлаОбмена = "";

КонецПроцедуры

'''
mark='// Обработчик события Нажатие кнопки Выполнить'
assert t.count(mark)==1
t=t.replace(mark,add+mark)
open(m,'w',encoding='utf-8',newline='').write(t)
