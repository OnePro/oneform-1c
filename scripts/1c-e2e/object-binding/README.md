# Live check of object-attribute bindings (task A1)

Scripts used to verify in real 1C that new and changed fields are bound to attributes of the object
(catalog, document, external processor). They need Windows, 1C 8.3 and a test infobase; paths inside are the ones of the
author's machine and must be adjusted.

1. `oneform extract` an object of the configuration export, edit `Form.xml` (`edit_cat.py` adds four fields
   bound to `СправочникОбъект.Код`, `.Наименование` and two custom attributes), `oneform pack`.
2. `load.sh <config dir> <list file> <tag>`: partial load of the object into the infobase (`db-load-xml` skill) and `UpdateDBCfg`.
3. `build_launcher.py` builds `a1-launcher.epf` from `launcher_module.bsl` (swaps the module inside `target/dump-ws/dump-cfg.epf`).
4. `live.py <Вид.Имя> <Форма> first|new <out.txt> [shot.png]`: opens the form of an existing (`first`) or new object in the
   infobase and writes `pre:<element>.<property>=<value>` lines for every element, including `Данные` (the binding) and
   `Значение` (what the field shows).
5. `cmp.py <before.txt> <after.txt> <new names> pre`: nothing may change on the existing controls.

Result (26.10.2026): catalog and document forms of a test configuration, a test processor — runtime `Данные` of the new
controls equals the requested path and the fields show the values of an existing object; the other controls are unchanged.
