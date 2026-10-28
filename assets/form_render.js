/* OneFormRender: the single renderer of ordinary 1C forms (Form.xml -> HTML) shared by FormPreview.html and .oneform/index.html. */
(function (root) {
'use strict';

// ------------------------------------------------------------------ helpers
function esc(s) { return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;'); }
function kid(e, t) { if (!e) return null; for (var n = e.firstElementChild; n; n = n.nextElementSibling) if (n.tagName === t) return n; return null; }
function kids(e, t) { var r = []; if (!e) return r; for (var n = e.firstElementChild; n; n = n.nextElementSibling) if (!t || n.tagName === t) r.push(n); return r; }
function txt(e, t) { var k = kid(e, t); return k ? (k.textContent || '').trim() : ''; }
function num(e, t) { var v = parseInt(txt(e, t), 10); return isNaN(v) ? 0 : v; }
function loc(e, t) { var m = {}; kids(kid(e, t), 'Item').forEach(function (i) { m[i.getAttribute('language') || ''] = i.textContent || ''; }); return m; }
function T(m, lang) { return m && m[lang] !== undefined ? m[lang] : ''; }
function vtype(e) {
  var v = kid(e, 'ValueType');
  return v ? { kind: v.getAttribute('kind') || '', composite: v.getAttribute('composite') === 'true', fd: v.getAttribute('fractionDigits') } : null;
}
function fmt(n, d) {
  var p = new Array(d + 1).join('  '), a = '', i;
  for (i = 0; i < n.attributes.length; i++) a += ' ' + n.attributes[i].name + '="' + n.attributes[i].value + '"';
  if (n.tagName === 'ChildItems' && d > 0) return p + '<ChildItems>...</ChildItems>';
  var ch = kids(n);
  if (!ch.length) { var t = (n.textContent || '').trim(); return p + '<' + n.tagName + a + (t ? '>' + t + '</' + n.tagName + '>' : '/>'); }
  return p + '<' + n.tagName + a + '>\n' + ch.map(function (c) { return fmt(c, d + 1); }).join('\n') + '\n' + p + '</' + n.tagName + '>';
}

// ------------------------------------------------------------------ icons (inline SVG, no external resources)
function sv(w, inner, vb) { return '<svg width="' + w + '" height="' + w + '" viewBox="0 0 ' + (vb || w) + ' ' + (vb || w) + '">' + inner + '</svg>'; }
// Icons of render-spec §0.5 (inline SVG, no external resources) + viewer shell icons (eye, eyeOff, layers) + legacy landscape placeholder.
var ICON = {
  choice: '<svg width="12" height="12" viewBox="0 0 12 12"><rect x="1" y="8" width="2" height="2" fill="#664C00"/><rect x="5" y="8" width="2" height="2" fill="#664C00"/><rect x="9" y="8" width="2" height="2" fill="#664C00"/></svg>',
  calendar: '<svg width="12" height="12" viewBox="0 0 12 12"><rect x="1.5" y="1.5" width="9" height="9" fill="#fff" stroke="#664C00"/><path d="M1.5 4.5h9M4.5 4.5v6M7.5 4.5v6M1.5 7.5h9" stroke="#887437"/></svg>',
  calc: '<svg width="12" height="12" viewBox="0 0 12 12"><rect x="1.5" y="1.5" width="9" height="9" fill="#fff" stroke="#664C00"/><rect x="3" y="3" width="6" height="2" fill="#887437"/><path d="M3 7h1.5M5.25 7h1.5M7.5 7h1.5M3 9h1.5M5.25 9h1.5M7.5 9h1.5" stroke="#664C00"/></svg>',
  type: '<svg width="12" height="12" viewBox="0 0 12 12"><path d="M2 2.5h8v2H7v6H5v-6H2z" fill="#3D3D3D"/></svg>',
  clear: '<svg width="12" height="12" viewBox="0 0 12 12"><path d="M3 3l6 6M9 3l-6 6" stroke="#664C00" stroke-width="1.8"/></svg>',
  open: '<svg width="12" height="12" viewBox="0 0 12 12"><circle cx="5" cy="5" r="3.3" fill="#fff" stroke="#5A5A5A" stroke-width="1.4"/><path d="M7.4 7.4L10.6 10.6" stroke="#5A5A5A" stroke-width="2"/></svg>',
  drop: '<svg width="7" height="4" viewBox="0 0 7 4"><path d="M0 0h7L3.5 4z" fill="#3D3D3D"/></svg>',
  spin: '<svg width="7" height="9" viewBox="0 0 7 9"><path d="M0 3.5h7L3.5 0zM0 5.5h7L3.5 9z" fill="#3D3D3D"/></svg>',
  check: '<svg width="9" height="8" viewBox="0 0 9 8"><path d="M1 4l2.5 2.5L8 1.5" fill="none" stroke="#2B2B2B" stroke-width="2"/></svg>',
  sortAsc: '<svg width="7" height="4" viewBox="0 0 7 4"><path d="M0 4h7L3.5 0z" fill="#8C8670"/></svg>',
  treePlus: '<svg width="9" height="9" viewBox="0 0 9 9"><rect x=".5" y=".5" width="8" height="8" fill="#fff" stroke="#A9A9A9"/><path d="M2 4.5h5M4.5 2v5" stroke="#333"/></svg>',
  folder: '<svg width="16" height="16" viewBox="0 0 16 16"><path d="M1.5 4.5h5l1 1.5h7v7.5h-13z" fill="#F7D774" stroke="#C9A227"/><path d="M1.5 6.5h13" stroke="#E8BE3C"/></svg>',
  item: '<svg width="16" height="16" viewBox="0 0 16 16"><rect x="3" y="7" width="10" height="3" rx="1" fill="#7FA6D6" stroke="#3A6EA5"/></svg>',
  arrowLeft: '<svg width="5" height="7" viewBox="0 0 5 7"><path d="M4.5 0v7L0 3.5z" fill="#A8A38E"/></svg>',
  arrowRight: '<svg width="5" height="7" viewBox="0 0 5 7"><path d="M.5 0v7L5 3.5z" fill="#A8A38E"/></svg>',
  arrowUp: '<svg width="7" height="5" viewBox="0 0 7 5"><path d="M0 4.5h7L3.5 0z" fill="#A8A38E"/></svg>',
  arrowDown: '<svg width="7" height="5" viewBox="0 0 7 5"><path d="M0 .5h7L3.5 5z" fill="#A8A38E"/></svg>',
  tabLeft: '<svg width="5" height="7" viewBox="0 0 5 7"><path d="M4.5 0v7L0 3.5z" fill="#3D3D3D"/></svg>',
  tabRight: '<svg width="5" height="7" viewBox="0 0 5 7"><path d="M.5 0v7L5 3.5z" fill="#3D3D3D"/></svg>',
  picture: '<svg width="16" height="16" viewBox="0 0 16 16"><rect x="1.5" y="2.5" width="13" height="11" fill="#fff" stroke="#B3AC86"/><path d="M3 12l3.5-4 2.5 3 2-2 2 3z" fill="#C9C2A0"/><circle cx="11" cy="5.5" r="1.3" fill="#E8C34A"/></svg>',
  user: '<svg width="16" height="16" viewBox="0 0 16 16"><circle cx="8" cy="5" r="2.8" fill="#E8A33C" stroke="#9A5B00"/><path d="M2.5 15c0-3.2 2.5-5 5.5-5s5.5 1.8 5.5 5z" fill="#4C7BC5" stroke="#274A85"/></svg>',
  add: '<svg width="16" height="16" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6.5" fill="#43A047" stroke="#2E7D32"/><path d="M8 4.5v7M4.5 8h7" stroke="#fff" stroke-width="2"/></svg>',
  addGroup: '<svg width="16" height="16" viewBox="0 0 16 16"><path d="M1.5 4.5h5l1 1.5h7v7.5h-13z" fill="#F7D774" stroke="#C9A227"/><circle cx="11.5" cy="11.5" r="3.5" fill="#43A047"/><path d="M11.5 9.5v4M9.5 11.5h4" stroke="#fff" stroke-width="1.4"/></svg>',
  copy: '<svg width="16" height="16" viewBox="0 0 16 16"><rect x="2.5" y="1.5" width="8" height="10" fill="#fff" stroke="#6B6B6B"/><rect x="5.5" y="4.5" width="8" height="10" fill="#fff" stroke="#6B6B6B"/><circle cx="12" cy="12.5" r="3" fill="#43A047"/><path d="M12 11v3M10.5 12.5h3" stroke="#fff"/></svg>',
  edit: '<svg width="16" height="16" viewBox="0 0 16 16"><path d="M3 13l1-3.5 7-7 2.5 2.5-7 7z" fill="#8BC34A" stroke="#4E7A1E"/><path d="M3 13l1-3.5 2.5 2.5z" fill="#F0D9A0" stroke="#4E7A1E"/></svg>',
  delete: '<svg width="16" height="16" viewBox="0 0 16 16"><rect x="2.5" y="2.5" width="11" height="11" fill="#fff" stroke="#B0B0B0"/><path d="M5 5l6 6M11 5l-6 6" stroke="#E53935" stroke-width="2"/></svg>',
  save: '<svg width="16" height="16" viewBox="0 0 16 16"><rect x="2.5" y="2.5" width="11" height="11" fill="#7A92B8" stroke="#3B5579"/><rect x="5" y="2.5" width="6" height="4" fill="#fff"/><rect x="5" y="9" width="6" height="4.5" fill="#DCDCDC"/></svg>',
  up: '<svg width="16" height="16" viewBox="0 0 16 16"><path d="M8 2.5l5 5h-3v6H6v-6H3z" fill="#5C8FD6" stroke="#2F5F9A"/></svg>',
  down: '<svg width="16" height="16" viewBox="0 0 16 16"><path d="M8 13.5l5-5h-3v-6H6v6H3z" fill="#5C8FD6" stroke="#2F5F9A"/></svg>',
  sortAz: '<svg width="16" height="16" viewBox="0 0 16 16"><text x="1" y="7" font-size="7" font-family="Tahoma" font-weight="bold" fill="#333">А</text><text x="1" y="15" font-size="7" font-family="Tahoma" font-weight="bold" fill="#333">Я</text><path d="M12 3v10M9.5 10.5L12 13l2.5-2.5" stroke="#2F6FD0" fill="none" stroke-width="1.4"/></svg>',
  sortZa: '<svg width="16" height="16" viewBox="0 0 16 16"><text x="1" y="7" font-size="7" font-family="Tahoma" font-weight="bold" fill="#333">Я</text><text x="1" y="15" font-size="7" font-family="Tahoma" font-weight="bold" fill="#333">А</text><path d="M12 3v10M9.5 5.5L12 3l2.5 2.5" stroke="#2F6FD0" fill="none" stroke-width="1.4"/></svg>',
  filter: '<svg width="16" height="16" viewBox="0 0 16 16"><path d="M2.5 3h11l-4 5v5l-3-1.5V8z" fill="#9FC3E8" stroke="#2F5F9A"/></svg>',
  filterValue: '<svg width="16" height="16" viewBox="0 0 16 16"><path d="M2.5 3h11l-4 5v5l-3-1.5V8z" fill="#F2C94C" stroke="#8A6D00"/></svg>',
  filterHistory: '<svg width="16" height="16" viewBox="0 0 16 16"><path d="M2.5 3h11l-4 5v5l-3-1.5V8z" fill="#9FC3E8" stroke="#2F5F9A"/><path d="M12.5 10v5M10 12.5h5" stroke="#333"/></svg>',
  filterOff: '<svg width="16" height="16" viewBox="0 0 16 16"><path d="M2.5 3h11l-4 5v5l-3-1.5V8z" fill="#E6E6E6" stroke="#8A8A8A"/><path d="M9.5 9.5l5 5M14.5 9.5l-5 5" stroke="#E53935" stroke-width="1.6"/></svg>',
  refresh: '<svg width="16" height="16" viewBox="0 0 16 16"><path d="M13 8a5 5 0 1 1-1.5-3.6" fill="none" stroke="#43A047" stroke-width="2"/><path d="M13.5 1.5v4h-4z" fill="#43A047"/></svg>',
  outputList: '<svg width="16" height="16" viewBox="0 0 16 16"><rect x="2.5" y="2.5" width="11" height="11" fill="#fff" stroke="#6B6B6B"/><path d="M4.5 5.5h7M4.5 8h7M4.5 10.5h4" stroke="#8A8A8A"/><circle cx="12" cy="12" r="2.6" fill="#43A047"/></svg>',
  hierarchy: '<svg width="16" height="16" viewBox="0 0 16 16"><rect x="1.5" y="2.5" width="6" height="4" fill="#F7D774" stroke="#C9A227"/><rect x="8.5" y="9.5" width="6" height="4" fill="#fff" stroke="#6B6B6B"/><path d="M4.5 6.5v5h4" fill="none" stroke="#6B6B6B"/></svg>',
  help: '<svg width="16" height="16" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6.5" fill="#F0A830" stroke="#B26B00"/><text x="5.3" y="11.6" font-size="9.5" font-weight="bold" font-family="Tahoma" fill="#fff">?</text></svg>',
  movements: '<svg width="16" height="16" viewBox="0 0 16 16"><text x=".5" y="7.5" font-size="6.5" font-weight="bold" font-family="Tahoma" fill="#2F5F9A">Дт</text><text x="6" y="14.5" font-size="6.5" font-weight="bold" font-family="Tahoma" fill="#B03030">Кт</text></svg>',
  print: '<svg width="16" height="16" viewBox="0 0 16 16"><rect x="4.5" y="1.5" width="7" height="5" fill="#fff" stroke="#6B6B6B"/><rect x="1.5" y="6.5" width="13" height="6" fill="#B8C4D6" stroke="#55657F"/><rect x="4.5" y="10.5" width="7" height="4" fill="#fff" stroke="#6B6B6B"/></svg>',
  run: '<svg width="16" height="16" viewBox="0 0 16 16"><path d="M4 2.5l9 5.5-9 5.5z" fill="#43A047" stroke="#2E7D32"/></svg>',
  cfgMarker: '<svg width="16" height="16" viewBox="0 0 16 16"><rect x="2.5" y="2.5" width="11" height="11" rx="3" fill="none" stroke="#333" stroke-dasharray="2 1.5"/></svg>',
  eye: sv(12, '<path d="M1 6c1.6-3 3.3-4.2 5-4.2S9.4 3 11 6c-1.6 3-3.3 4.2-5 4.2S2.6 9 1 6z" fill="#fff" stroke="#3A6EA5"/><circle cx="6" cy="6" r="1.8" fill="#1c5ccc"/>'),
  eyeOff: sv(12, '<path d="M1 6c1.6-3 3.3-4.2 5-4.2S9.4 3 11 6c-1.6 3-3.3 4.2-5 4.2S2.6 9 1 6z" fill="#f0f0f0" stroke="#999"/><path d="M2 10L10 2" stroke="#c33" stroke-width="1.6"/>'),
  layers: sv(12, '<path d="M6 1.5L11 4 6 6.5 1 4z" fill="#FFF3B8" stroke="#9A6B00"/><path d="M1 6.5L6 9l5-2.5M1 9l5 2.5L11 9" fill="none" stroke="#9A6B00"/>'),
};
var AUTO_FORM = ['help'];
var AUTO_FORM_EXT = ['act_menu', '|', 'help'];
// AutoFill sets by place of the bar (render-spec §13); '!' = unavailable button
var AUTO_FORM_ITEM = ['act_menu', '|', 'save!', 'refresh!', 'copy', '|', 'goto', '|', 'help'];
var AUTO_FORM_LIST = ['act_menu', 'add', 'addGroup', 'copy', 'edit', 'delete', '|', 'hierarchy', '|', 'filter', 'filterValue', 'filterOff', '|', 'refresh', '|', 'goto', '|', 'help'];
var AUTO_TABLE = ['add', 'copy', 'edit', 'delete', '|', 'save', 'up', 'down', 'sortAz', 'sortZa', '|', 'filter', 'filterValue', 'filterHistory', 'filterOff', '|', 'outputList'];
var AUTO_BOTTOM = ['ok', '|', 'write', '|', 'close'];
var AUTO_BOTTOM_DOC = ['ok', '|', 'post', '|', 'write', '|', 'close'];
var AUTO_META = {
  act_menu: { name: 'Действия', title: 'Действия', action: 'Standard:ActionsSubmenu', textSubmenu: true },
  goto: { name: 'Перейти', title: 'Перейти', action: 'Standard:GoToSubmenu', textSubmenu: true },
  ok: { name: 'ОК', title: 'OK', action: 'Standard:141', textBtn: true, okBtn: true },
  write: { name: 'Записать', title: 'Записать', action: 'Standard:140', textBtn: true },
  post: { name: 'Провести', title: 'Провести', action: 'Standard:142', textBtn: true },
  close: { name: 'Закрыть', title: 'Закрыть', action: 'Standard:31', textBtn: true },
  add: { name: 'Добавить', title: 'Добавить строку (Ins)', action: 'Standard:AddRow' },
  addGroup: { name: 'ДобавитьГруппу', title: 'Добавить группу (Ctrl+F9)', action: 'Standard:AddGroup' },
  copy: { name: 'Скопировать', title: 'Скопировать (F9)', action: 'Standard:CopyRow' },
  edit: { name: 'Изменить', title: 'Изменить текущий элемент (F2)', action: 'Standard:EditRow' },
  delete: { name: 'Удалить', title: 'Удалить текущий (Del)', action: 'Standard:DeleteRow' },
  save: { name: 'ЗакончитьРедактирование', title: 'Закончить редактирование строки', action: 'Standard:EndEdit' },
  up: { name: 'ПереместитьВверх', title: 'Переместить вверх', action: 'Standard:MoveUp' },
  down: { name: 'ПереместитьВниз', title: 'Переместить вниз', action: 'Standard:MoveDown' },
  sortAz: { name: 'СортироватьПоВозрастанию', title: 'Сортировать по возрастанию', action: 'Standard:SortAsc' },
  sortZa: { name: 'СортироватьПоУбыванию', title: 'Сортировать по убыванию', action: 'Standard:SortDesc' },
  filter: { name: 'УстановитьОтбор', title: 'Установить отбор и сортировку списка…', action: 'Standard:SetFilter' },
  filterValue: { name: 'ОтборПоЗначению', title: 'Отбор по значению в текущей колонке', action: 'Standard:FilterByValue' },
  filterHistory: { name: 'ИсторияОтборов', title: 'История отборов', action: 'Standard:FilterHistory' },
  filterOff: { name: 'ОтключитьОтбор', title: 'Отключить отбор', action: 'Standard:ClearFilter' },
  outputList: { name: 'ВывестиСписок', title: 'Вывести список…', action: 'Standard:OutputList' },
  hierarchy: { name: 'ИерархическийПросмотр', title: 'Иерархический просмотр', action: 'Standard:Hierarchy' },
  refresh: { name: 'Обновить', title: 'Обновить (Ctrl+Shift+R)', action: 'Standard:146' },
  help: { name: 'Справка', title: 'Открыть справку (F1)', action: 'Standard:300' }
};
var STD_TABLE_SKIP = { 'Standard:53': 1, 'Standard:91': 1, 'Standard:93': 1, 'Standard:97': 1 };
var STD_ICON = { 'Standard:147': 'open', 'Standard:146': 'refresh', 'Standard:145': 'copy', 'Standard:300': 'help' };
var STD_NAMES = {
  'Standard:31': 'Закрыть',
  'Standard:140': 'Записать',
  'Standard:141': 'ОК (Записать и закрыть)',
  'Standard:142': 'Провести',
  'Standard:145': 'Скопировать',
  'Standard:146': 'Обновить',
  'Standard:147': 'Найти в списке',
  'Standard:300': 'Справка'
};

// ------------------------------------------------------------------ parsing
function parse(text) {
  var doc = new DOMParser().parseFromString(text, 'application/xml');
  if (doc.getElementsByTagName('parsererror').length) return { error: 'Form.xml не разобран' };
  var root = doc.documentElement, attrs = {}, A = [], C = [], Cols = [];
  var mainAttrEl = kid(root, 'MainAttribute'), mainAttrId = mainAttrEl ? (mainAttrEl.getAttribute('attributeId') || '') : '';
  kids(kid(root, 'Attributes'), 'Attribute').forEach(function (a) {
    var aid = a.getAttribute('id') || '', aname = a.getAttribute('name') || '';
    attrs[aid] = aname;
    A.push({
      ak: A.length, id: aid, name: aname,
      main: mainAttrId ? (aid === mainAttrId) : (txt(a, 'Main') === 'true'), stored: txt(a, 'StoredData') === 'true',
      vt: vtype(a), xml: fmt(a, 0)
    });
  });
  function walk(e, parent, ox, oy, pi, depth, pd) {
    var l = num(e, 'Left'), t = num(e, 'Top'), k = C.length, dp = kid(e, 'DataPath'), aid = dp ? (dp.getAttribute('attributeId') || '') : '', st = kid(e, 'ShowTabs');
    var isPage = e.tagName === 'Page', ax = ox + l, ay = oy + t;
    C.push({
      k: k, tag: e.tagName, e: e, name: e.getAttribute('name') || '', id: e.getAttribute('id') || (isPage ? 'page-' + pi : ''), pi: pi,
      showTabs: st ? (st.getAttribute('code') || '0') : '', parent: parent, depth: depth, pd: pd, l: l, t: t, w: num(e, 'Width'), h: num(e, 'Height'), z: num(e, 'ZOrder'),
      al: ax, at: ay, ax: ax + pd, ay: ay + pd, ps: k, tabless: false,
      visible: txt(e, 'Visible') !== 'false', pr: elProps(e), caps: loc(e, isPage ? 'Title' : 'Caption'),
      dp: aid || (dp ? (dp.textContent || '').trim() : ''), dpName: (dp && (dp.textContent || '').trim()) || attrs[aid] || '',
      cols: kids(kid(e, 'Columns'), 'Column').map(function (c2) {
        var colk = Cols.length, cdp = kid(c2, 'DataPath'), caid = cdp ? (cdp.getAttribute('attributeId') || '') : '';
        var colObj = {
          colk: colk, ck: k, e: c2, name: c2.getAttribute('name') || '', id: c2.getAttribute('id') || '',
          headers: loc(c2, 'Header'), footers: loc(c2, 'FooterText'), tips: loc(c2, 'Tooltip'),
          editor: txt(c2, 'Editor'), w: num(c2, 'Width') || 75,
          visible: txt(c2, 'Visible') !== 'false', ro: txt(c2, 'ReadOnly') === 'true', vt: vtype(c2),
          dp: caid, dpName: (cdp && (cdp.textContent || '').trim()) || '',
          events: kids(kid(c2, 'Events')).map(function (v) { return { name: v.tagName, handler: (v.textContent || '').trim() }; }),
          xml: fmt(c2, 0)
        };
        Cols.push(colObj);
        return colObj;
      }),
      choiceList: kids(kid(e, 'ChoiceList'), 'ChoiceItem').map(function (ci) {
        return { value: ci.getAttribute('value') || '', pres: loc(ci, 'Presentation') };
      }),
      events: kids(kid(e, 'Events')).map(function (v) { return { name: v.tagName, handler: (v.textContent || '').trim() }; }),
      xml: fmt(e, 0)
    });
    var np = 0, nextPd = pd + (isPage ? 1 : 0);
    kids(kid(e, 'ChildItems')).forEach(function (c) { var cp = c.tagName === 'Page'; walk(c, k, ax, ay, cp ? np : -1, depth + 1, nextPd); if (cp) np++; });
  }
  var rootNp = 0, rst = kid(root, 'ShowTabs'), rootShowTabs = rst ? (rst.getAttribute('code') || '0') : '', isExt = !!kid(root, 'DataProcessorFormExtension');
  kids(kid(root, 'ChildItems')).forEach(function (c) { var cp = c.tagName === 'Page'; walk(c, -1, 0, 0, cp ? rootNp : -1, 0, 0); if (cp) rootNp++; });
  var kidsOf = {}, ov = {}, members = {}, tablessPanels = [], psSeq = 0;
  C.forEach(function (c) { (kidsOf[c.parent] = kidsOf[c.parent] || []).push(c); ov[c.k] = []; });
  function assignPs(p) {
    ordered(kidsOf[p]).forEach(function (c) { c.ps = ++psSeq; assignPs(c.k); });
  }
  assignPs(-1);
  C.forEach(function (c) {
    if (c.tag !== 'Panel') return;
    var pgs = (kidsOf[c.k] || []).filter(function (x) { return x.tag === 'Page'; });
    if (pgs.length > 1 && tabStrip(c.w, c.h, pgs[0], c.showTabs).mode === 'none') { c.tabless = true; tablessPanels.push(c); }
  });
  // first radio button of a group (same parent and DataPath): the sample value of the designer view (render-spec §7)
  var radioFirst = {};
  C.forEach(function (c) {
    if (c.tag !== 'RadioButton' || !c.dp || !kid(c.e, 'SelectionValue')) return;
    var key = c.parent + '|' + c.dp, f = radioFirst[key], ord = num(c.e, 'TabOrder') || c.z;
    if (!f || ord < f.ord) radioFirst[key] = { c: c, ord: ord };
  });
  Object.keys(radioFirst).forEach(function (key) { radioFirst[key].c.radioFirst = true; });
  Object.keys(kidsOf).forEach(function (pk) {
    var sibs = ordered(kidsOf[pk]).filter(function (c) { return c.tag !== 'Page' && c.w > 0 && c.h > 0; });
    for (var i = 0; i < sibs.length; i++) {
      for (var j = i + 1; j < sibs.length; j++) {
        var a = sibs[i], b = sibs[j];
        var ix = Math.min(a.l + a.w, b.l + b.w) - Math.max(a.l, b.l);
        var iy = Math.min(a.t + a.h, b.t + b.h) - Math.max(a.t, b.t);
        if (ix < 4 || iy < 4) continue;
        var ia = ix * iy, aa = a.w * a.h, ba = b.w * b.h, minA = Math.min(aa, ba);
        if (a.tag === 'UsualGroup' && b.tag !== 'UsualGroup' && ia >= 0.85 * ba && aa > ba * 1.15) {
          (members[a.k] = members[a.k] || []).push(b.k); continue;
        }
        if (b.tag === 'UsualGroup' && a.tag !== 'UsualGroup' && ia >= 0.85 * aa && ba > aa * 1.15) {
          (members[b.k] = members[b.k] || []).push(a.k); continue;
        }
        if (ia < 0.5 * minA) continue;
        var exact = Math.abs(a.l - b.l) <= 2 && Math.abs(a.t - b.t) <= 2 && Math.abs(a.w - b.w) <= 2 && Math.abs(a.h - b.h) <= 2;
        var kind = exact ? 'exact' : 'partial';
        ov[a.k].push({ k: b.k, kind: kind, above: true });
        ov[b.k].push({ k: a.k, kind: kind, above: false });
      }
    }
  });
  var seen = {};
  var B = [], btnsOf = {}, autoBtnsOf = {};
  function walkBtns(e, ck, parentBk, depth) {
    return kids(kid(e, 'Buttons'), 'Button').map(function (b) {
      var bk = B.length;
      var item = {
        bk: bk, ck: ck, parentBk: parentBk, depth: depth, auto: false,
        name: b.getAttribute('name') || '', id: b.getAttribute('id') || '',
        kind: b.getAttribute('kind') || 'Action', checked: b.getAttribute('checked') === 'true',
        action: b.getAttribute('action') || '', actionKind: b.getAttribute('actionKind') || '',
        rep: b.getAttribute('representation') || 'Auto', pic: b.getAttribute('hasPicture') === 'true',
        dis: b.getAttribute('enabled') === 'false' || txt(b, 'Enabled') === 'false', def: txt(b, 'DefaultButton') === 'true',
        caps: loc(b, 'Caption'), tips: loc(b, 'Tooltip'), kids: [], xml: fmt(b, 0)
      };
      B.push(item);
      item.kids = walkBtns(b, ck, bk, depth + 1);
      return item;
    });
  }
  function isFormLevelBar(c) {
    if (c.parent < 0) return true;
    var p = C[c.parent];
    return !!(p && p.tag === 'Page' && p.parent < 0 && c.w >= p.w - 4 && (c.t <= 4 || c.t + c.h >= p.h - 4));
  }
  var formH = num(root, 'Height'), isDoc = /DocumentObject|DocumentList|ДокументОбъект|ДокументСписок/.test(text);
  // a list form: a table bound to the main attribute itself (not to a tabular part "Объект.Часть")
  var isListForm = !!mainAttrId && C.some(function (t) { return t.tag === 'Table' && t.dp === mainAttrId && t.dpName.indexOf('.') < 0; });
  C.forEach(function (t) { if (t.tag === 'Table' && !!mainAttrId && t.dp === mainAttrId && t.dpName.indexOf('.') < 0) t.mainList = true; });
  C.forEach(function (c) {
    if (c.tag === 'CommandBar') {
      btnsOf[c.k] = walkBtns(c.e, c.k, -1, 1);
      var auto = txt(c.e, 'AutoFill') === 'true', formBar = isFormLevelBar(c), noOwn = !btnsOf[c.k].length, autoCodes;
      if (!formBar) autoCodes = AUTO_TABLE;
      else if (txt(c.e, 'HorizontalAlign') === 'Right' && c.t + c.h / 2 > (c.parent >= 0 ? C[c.parent].h : formH) / 2) autoCodes = noOwn ? (isDoc ? AUTO_BOTTOM_DOC : AUTO_BOTTOM) : [];
      else if (!noOwn) autoCodes = AUTO_FORM;
      else autoCodes = isExt ? AUTO_FORM_EXT : isListForm ? AUTO_FORM_LIST : AUTO_FORM_ITEM;
      autoBtnsOf[c.k] = auto ? autoCodes.map(function (code2) {
        if (code2 === '|') return { sep: true };
        var dis = code2.charAt(code2.length - 1) === '!';
        if (dis) code2 = code2.slice(0, -1);
        var am = AUTO_META[code2] || { name: code2, title: code2, action: '' };
        var bk = B.length;
        var item = {
          bk: bk, ck: c.k, parentBk: -1, depth: 1, auto: true, icoKey: code2, textSubmenu: !!am.textSubmenu, textBtn: !!am.textBtn, okBtn: !!am.okBtn, dis: dis,
          name: am.name, id: '', kind: am.textSubmenu ? 'Submenu' : 'Action', checked: false,
          action: am.action, actionKind: 'AutoFill', rep: am.textSubmenu || am.textBtn ? 'Text' : 'Picture', pic: !am.textSubmenu && !am.textBtn,
          caps: { ru: am.title }, tips: { ru: am.title }, kids: [], xml: '<!-- Стандартная кнопка автозаполнения (' + am.name + ') -->'
        };
        B.push(item);
        return item;
      }) : [];
    }
    Object.keys(c.caps).forEach(function (l) { seen[l] = 1; });
    c.cols.forEach(function (x) { Object.keys(x.headers).forEach(function (l) { seen[l] = 1; }); });
  });
  B.forEach(function (b) {
    Object.keys(b.caps).forEach(function (l) { seen[l] = 1; });
    Object.keys(b.tips).forEach(function (l) { seen[l] = 1; });
  });
  Object.keys(loc(root, 'Caption')).forEach(function (l) { seen[l] = 1; });
  var langs = ['ru', 'ro'];
  Object.keys(seen).sort().forEach(function (l) { if (l && langs.indexOf(l) < 0) langs.push(l); });
  return {
    root: root, A: A, C: C, Cols: Cols, B: B, btnsOf: btnsOf, autoBtnsOf: autoBtnsOf, kidsOf: kidsOf, ov: ov, members: members, tablessPanels: tablessPanels,
    formId: root.getAttribute('id') || '', formName: root.getAttribute('name') || '', showTabs: rootShowTabs, isExt: isExt,
    langs: langs, width: num(root, 'Width'), height: num(root, 'Height'), caps: loc(root, 'Caption'), capPic: !!kid(root, 'CaptionPicture'),
    events: kids(kid(root, 'Events')).map(function (v) { return { name: v.tagName, handler: (v.textContent || '').trim() }; })
  };
}

// ------------------------------------------------------------------ rendering
function ordered(list) {
  return (list || []).map(function (c, i) { return [c, i]; })
    .sort(function (a, b) { return (a[0].z - b[0].z) || (a[1] - b[1]); }).map(function (p) { return p[0]; });
}
function vis(c, R) {
  if (R && R.visOverride && c.k in R.visOverride) return R.visOverride[c.k];
  return c.visible || !!(R && R.showHidden);
}
function isPageChainActive(c, R) {
  for (var p = c.parent; p >= 0; p = R.m.C[p].parent) {
    var anc = R.m.C[p];
    if (anc.tag === 'Page') {
      if (R.active[anc.parent] !== anc.k) return false;
    } else if (!vis(anc, R)) return false;
  }
  return true;
}
function pos(c) { return 'left:' + c.l + 'px;top:' + c.t + 'px;width:' + c.w + 'px;height:' + c.h + 'px'; }
// ---- shared look helpers (render-spec §1): one translation of Font / Color / Border / Picture / alignment / Tooltip
function hexOf(v) {
  v = String(v || '').trim();
  var m = /^rgb\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)$/i.exec(v) || /^(\d+)\s*,\s*(\d+)\s*,\s*(\d+)$/.exec(v);
  if (m) return '#' + [m[1], m[2], m[3]].map(function (x) { return ('0' + Math.min(255, +x).toString(16)).slice(-2); }).join('').toUpperCase();
  return v;
}
// colour of <tag kind="rgb|style|auto" value="#RRGGBB"/>: rgb -> value; style -> value when present (without value: absent); auto/unknown -> absent
function colorCss(e, tag) {
  var n = kid(e, tag);
  if (!n) return '';
  var kind = n.getAttribute('kind') || '', v = n.getAttribute('value') || '';
  if (!v || kind === 'auto' || (kind && kind !== 'rgb' && kind !== 'style')) return '';
  return hexOf(v);
}
var colorOf = colorCss;
// <Font face bold italic underline strikeout size|height scale/> -> {cls: ' b f-i f-u f-s', st: ';font-family:..;font-size:..', bold: true|false|null}
function fontSpec(e, tag) {
  var f = kid(e, tag || 'Font'), r = { cls: '', st: '', bold: null }, face, size, scale, pt;
  if (!f) return r;
  var b = f.getAttribute('bold');
  if (b === 'true') { r.cls += ' b'; r.bold = true; } else if (b === 'false') { r.st += ';font-weight:normal'; r.bold = false; }
  if (f.getAttribute('italic') === 'true') r.cls += ' f-i';
  if (f.getAttribute('underline') === 'true') r.cls += ' f-u';
  if (f.getAttribute('strikeout') === 'true') r.cls += ' f-s';
  face = f.getAttribute('face');
  if (face) r.st += ";font-family:'" + face.replace(/['"\;<>{}]/g, '') + "',Tahoma,Arial,sans-serif";
  size = parseFloat(f.getAttribute('size') || f.getAttribute('height')); scale = parseFloat(f.getAttribute('scale'));
  pt = size > 0 ? size : 0;
  if (scale > 0 && scale !== 100) r.st += ';font-size:calc(' + (pt || 8) + 'pt * ' + scale + ' / 100)';
  else if (pt) r.st += ';font-size:' + pt + 'pt';
  return r;
}
function styleAttr(s) { return s ? ' style="' + esc(s.replace(/^;/, '')) + '"' : ''; }
function colorStyle(prop, v) { return v ? ';' + prop + ':' + v : ''; }
// BorderStyle: <BorderStyle code="4">SingleLine</BorderStyle> (or type="…") -> ' bd-<type>'
function borderType(e) {
  var bs = kid(e, 'BorderStyle'), t, cd;
  if (!bs) return '';
  t = (bs.textContent || '').trim() || bs.getAttribute('type') || '';
  if (!t) { cd = bs.getAttribute('code'); t = cd === '0' ? 'None' : cd === '4' ? 'SingleLine' : cd === '6' ? 'Frame' : ''; }
  return /^[A-Za-z0-9]+$/.test(t) ? t : '';
}
function alignCls(e, tag, pre) {
  var v = txt(e, tag);
  return /^(Left|Center|Right|Justify|Top|Bottom)$/.test(v) && (pre === 'ha' ? v !== 'Top' && v !== 'Bottom' : v !== 'Left' && v !== 'Right' && v !== 'Justify') ? ' ' + pre + '-' + v : '';
}
function hasPicture(e, tag) {
  var p = kid(e, tag || 'Picture');
  return !!p && (!!p.getAttribute('refGuid') || !!p.getAttribute('kind') || (p.textContent || '').trim() === 'true');
}
function pic16(name) { return '<span class="pic16">' + (name === 'НадписьАвтор' ? ICON.user : ICON.picture) + '</span>'; }
// properties common to every element, read once in parse()
function elProps(e) {
  return { en: txt(e, 'Enabled') !== 'false', ro: txt(e, 'ReadOnly') === 'true', tr: txt(e, 'TransparentBack') === 'true',
    bd: borderType(e), hasTip: !!kid(e, 'Tooltip'), tips: loc(e, 'Tooltip') };
}
// title= of the root: Tooltip text of the selected language (name stays in the inspector); without Tooltip the element name
function titleOf(c, R) { return c.pr && c.pr.hasTip ? T(c.pr.tips, R.lang) : c.name; }
var mctx = null;
function textW(s, font) {
  try { if (!mctx) mctx = document.createElement('canvas').getContext('2d'); mctx.font = font; return mctx.measureText(s).width; } catch (err) { return String(s).length * 6; }
}
function pic12() { return ICON.picture.replace('width="16" height="16"', 'width="12" height="12"'); }
function dateText(vt) {
  if (!vt) return '';
  if (vt.kind === 'DateTime') return ' . .   : :';
  if (vt.kind === 'Date') return ' . .';
  if (vt.kind === 'Time') return '  : :';
  return '';
}
function numText(d) { return d > 0 ? '0,' + new Array(d + 1).join('0') : '0'; }
// designer sample of an input field value: Mask, Format, ValueType (runtime: always empty)
function sampleOf(e, vt) {
  var mask = txt(e, 'Mask'), f = kid(e, 'Format'), ft = f ? ((kid(f, 'Item') || f).textContent || '') : '', m, r = { val: '', num: false };
  if (txt(e, 'PasswordMode') === 'true') { r.val = '*****'; return r; }
  if (mask) { r.val = mask.replace(/[9#NUX!@h]/g, ' '); return r; }
  if ((m = /ЧДЦ\s*=\s*(\d+)/.exec(ft))) { r.val = numText(+m[1]); r.num = true; return r; }
  if (/ДЛФ\s*=\s*T\b/.test(ft)) { r.val = '  : :'; return r; }
  if (/ДЛФ\s*=\s*DT|HH|mm/.test(ft)) { r.val = ' . .   : :'; return r; }
  if (/ДФ\s*=|ДЛФ\s*=/.test(ft)) { r.val = ' . .'; return r; }
  if (vt && vt.kind === 'Number') { r.num = true; r.val = vt.fd === null || vt.fd === undefined ? '0,00' : numText(parseInt(vt.fd, 10) || 0); return r; }
  r.val = dateText(vt);
  return r;
}

// InputField / ChoiceField (render-spec §3, §18)
function inputField(c, R) {
  var e = c.e, p = c.pr, des = R.mode === 'designer', vt = vtype(e), choice = c.tag === 'ChoiceField', fs = fontSpec(e, 'Font');
  var smp = !choice && des ? sampleOf(e, vt) : { val: '', num: false };
  var vcls = 'val' + (smp.num ? ' num' : '') + (txt(e, 'MultiLine') === 'true' ? ' ml' : '') + (!choice && des && txt(e, 'PasswordMode') === 'true' ? ' pw' : '') +
    alignCls(e, 'HorizontalAlign', 'ha') + alignCls(e, 'VerticalAlign', 'va') + fs.cls;
  var vst = colorStyle('color', colorCss(e, 'FieldTextColor') || colorCss(e, 'TextColor')) + fs.st;
  var tcls = (p.tr ? ' tr-bg' : '') + (colorCss(e, 'ButtonBackColor') ? ' colored' : '');
  var tst = colorStyle('--tb-bg', colorCss(e, 'ButtonBackColor')) + colorStyle('color', colorCss(e, 'ButtonTextColor'));
  var tb = [], tbAttr = styleAttr(tst);
  function add(extra, svg) { tb.push('<span class="tb' + extra + tcls + '"' + tbAttr + '>' + svg + '</span>'); }
  var cbTag = kid(e, 'ChoiceButton'), showChoice;
  if (choice) showChoice = !cbTag || txt(e, 'ChoiceButton') === 'true';
  else if (cbTag) showChoice = txt(e, 'ChoiceButton') === 'true';
  else showChoice = !!vt && (vt.kind === 'Ref' || vt.kind === 'Date' || vt.kind === 'DateTime' || vt.kind === 'Number');
  if (choice) {
    if (showChoice) add(' dd', kid(e, 'ChoiceButtonPicture') ? pic12() : ICON.drop);
  } else {
    if (txt(e, 'DropListButton') === 'true') add(' dd', ICON.drop);
    if (showChoice) {
      add('', kid(e, 'ChoiceButtonPicture') ? pic12() : vt && vt.composite ? ICON.type : vt && (vt.kind === 'DateTime' || vt.kind === 'Date') ? ICON.calendar : vt && vt.kind === 'Number' ? ICON.calc : ICON.choice);
    }
  }
  if (txt(e, 'ClearButton') === 'true') add('', ICON.clear);
  if (txt(e, 'OpenButton') === 'true') add('', ICON.open);
  if (txt(e, 'SpinButton') === 'true') add(' spin', '<i>▲</i><i>▼</i>');
  if (tb.length && c.w < tb.length * 16 + 12) tb = [];
  var vs = txt(e, 'VerticalScrollBar');
  var vsb = vs === 'UseAlways' || vs === 'true' ? '<span class="vsb"><span class="sbb">' + ICON.arrowUp + '</span><span class="sbb">' + ICON.arrowDown + '</span></span>' : '';
  var mark = txt(e, 'AutoMarkIncomplete') === 'true' ? '<i class="mark"></i>' : '';
  var back = colorCss(e, 'FieldBackColor') || colorCss(e, 'BackColor'), bc = colorCss(e, 'BorderColor');
  return {
    cls: (p.ro && p.en ? ' ro' : '') + (p.bd ? ' bd-' + p.bd : ''), st: colorStyle('background', back) + colorStyle('border-color', bc),
    inner: (hasPicture(e) ? pic16(c.name) : '') + '<span class="' + vcls + '"' + styleAttr(vst) + '>' + esc(smp.val) + mark + '</span>' + tb.join('') + vsb
  };
}

// LabelDecoration / small PictureDecoration (render-spec §4)
function label(c, R, cls, st) {
  var e = c.e, p = c.pr, des = R.mode === 'designer', cap = T(c.caps, R.lang), s = st, fs = fontSpec(e, 'Font');
  if (!cap && des && c.tag === 'LabelDecoration') cap = '<<' + c.name + '>>';
  var back = colorCss(e, 'BackColor'), vAl = txt(e, 'VerticalAlign'), link = txt(e, 'Hyperlink') === 'true', bc = colorCss(e, 'BorderColor');
  var ml = vAl === 'Top' || (!vAl && c.h >= 28), pl = txt(e, 'LabelPictureLocation');
  if (back) s += ';background:' + back; else if (des) s += ';background:var(--form-bg)';
  s += colorStyle('border-color', bc);
  cls += (link ? ' link' : '') + alignCls(e, 'HorizontalAlign', 'ha') + alignCls(e, 'VerticalAlign', 'va') + (ml ? ' ml' : '') +
    (/^(Right|Top|Bottom|Center)$/.test(pl) ? ' pl-' + pl : '') + (p.tr ? ' tr-bg' : '') + (p.bd ? ' bd-' + p.bd : '');
  var cst = colorStyle('color', colorCss(e, 'TextColor')) + fs.st;
  return { cls: cls, style: s, inner: (hasPicture(e) ? pic16(c.name) : '') + '<span class="cap' + fs.cls + '" data-k="' + c.k + '"' + styleAttr(cst) + '>' + esc(cap) + '</span>' };
}

// Button (render-spec §5)
function buttonBox(c, R) {
  var e = c.e, fs = fontSpec(e, 'Font'), cap = T(c.caps, R.lang), back = colorCss(e, 'ButtonBackColor'), bc = colorCss(e, 'BorderColor');
  var mn = /^(Use|UseExtra|true)$/.test(txt(e, 'MenuMode')) ? '<span class="mn">' + ICON.drop + '</span>' : '';
  var cst = colorStyle('color', colorCss(e, 'ButtonTextColor')) + fs.st;
  return {
    cls: (txt(e, 'DefaultButton') === 'true' ? ' def' : '') + (txt(e, 'MultiLine') === 'true' ? ' ml' : '') + (txt(e, 'PictureLocation') === 'Right' ? ' pl-Right' : '') +
      alignCls(e, 'HorizontalAlign', 'ha') + alignCls(e, 'VerticalAlign', 'va') + (back ? ' colored' : ''),
    st: colorStyle('--btn-bg', back) + colorStyle('border-color', bc),
    inner: (hasPicture(e) ? pic16(c.name) : '') + (cap && c.w >= 24 ? '<span class="cap' + fs.cls + '" data-k="' + c.k + '"' + styleAttr(cst) + '>' + esc(cap) + '</span>' : '') + mn
  };
}

// CheckBox / RadioButton (render-spec §6, §7)
function checkBox(c, R) {
  var e = c.e, p = c.pr, rad = c.tag === 'RadioButton', fs = fontSpec(e, 'Font'), tl = txt(e, 'TitleLocation');
  var on = rad && R.mode === 'designer' && !!c.radioFirst;
  var mix = !rad && txt(e, 'ThreeState') === 'true';
  var bst = colorStyle('border-color', colorCss(e, 'BorderColor')) + colorStyle('background', colorCss(e, 'FieldBackColor')) + colorStyle('--check-ink', colorCss(e, 'FieldTextColor'));
  var cst = colorStyle('color', colorCss(e, 'TextColor')) + fs.st, cap = T(c.caps, R.lang);
  return {
    cls: (tl === 'TitleLeft' || tl === 'Left' ? ' tl-Left' : '') + alignCls(e, 'HorizontalAlign', 'ha') + alignCls(e, 'VerticalAlign', 'va') + (p.tr ? ' tr-bg' : '') + (p.bd ? ' bd-' + p.bd : ''),
    st: colorStyle('background', colorCss(e, 'BackColor')),
    inner: '<span class="' + (rad ? 'rad' : 'box') + (on ? ' on' : '') + (mix ? ' mix' : '') + '"' + styleAttr(bst) + '></span>' +
      (cap ? '<span class="cap' + fs.cls + '" data-k="' + c.k + '"' + styleAttr(cst) + '>' + esc(cap) + '</span>' : '')
  };
}

// UsualGroup (render-spec §12)
function groupBox(c, R, cap) {
  var e = c.e, p = c.pr, bs = p.bd || (c.h > 20 ? 'Frame' : 'SingleLine'), fs = fontSpec(e, 'Font');
  var back = colorCss(e, 'BackColor'), bc = colorCss(e, 'BorderColor');
  if (bs !== 'None' && bs !== 'Frame' && bs !== 'SingleLine') bs = c.h > 20 ? 'Frame' : 'SingleLine';
  var gst = colorStyle('color', !p.en ? 'var(--text-dis)' : colorCss(e, 'TextColor')) + fs.st;
  var gt = cap ? '<span class="gt' + (p.tr ? ' tr-bg' : '') + fs.cls + '"' + styleAttr(gst) + '><span class="cap" data-k="' + c.k + '">' + esc(cap) + '</span></span>' : '';
  if (bs === 'None') return { cls: ' bs-None', st: colorStyle('background', back), inner: gt };
  return { cls: ' bs-' + bs, st: '', inner: '<i class="gf"' + styleAttr(colorStyle('border-color', bc) + colorStyle('background', back)) + '></i>' + gt };
}

// Table / TableColumn (render-spec §8, §9): header, sample row(s) of the designer view, footer, scroll bars, lines, colours, fonts
function tableBox(c, R) {
  var e = c.e, des = R.mode === 'designer', cols = c.cols.filter(function (x) { return x.visible || R.showHidden; });
  var hdrOff = txt(e, 'Header') === 'false', hn = Math.max(1, parseInt(txt(e, 'HeaderHeight'), 10) || 1), fn = Math.max(1, parseInt(txt(e, 'FooterHeight'), 10) || 1);
  var headH = hdrOff ? 0 : 20 + (hn - 1) * 13, footH = 20 + (fn - 1) * 13, footer = txt(e, 'Footer') === 'true';
  var tfs = fontSpec(e, 'Font'), hfs = fontSpec(e, 'HeaderFont'), ffs = fontSpec(e, 'FooterFont');
  var hBack = colorCss(e, 'HeaderBackColor'), hCol = colorCss(e, 'HeaderTextColor'), fBack = colorCss(e, 'FooterBackColor'), fCol = colorCss(e, 'FooterTextColor');
  var tText = colorCss(e, 'TextColor'), fldBack = colorCss(e, 'FieldBackColor'), fldText = colorCss(e, 'FieldTextColor');
  var selBack = colorCss(e, 'SelectionBackColor'), selText = colorCss(e, 'SelectionTextColor'), altBack = colorCss(e, 'AlternationRowBackColor');
  var hs = txt(e, 'HorizontalScrollBar'), vs = txt(e, 'VerticalScrollBar');
  var showSb = hs !== 'DontUse' && hs !== 'false', showVsb = vs === 'UseAlways' || vs === 'true';
  var cellMode = txt(e, 'RowSelectionMode') === 'Cell', alt = txt(e, 'UseAlternationRowColor') === 'true';
  var tree = !!kid(e, 'InitialTreeView') || cols.some(function (x) { return txt(x.e, 'ShowHierarchy') === 'true'; });
  var treeCol = cols.filter(function (x) { return txt(x.e, 'ShowHierarchy') === 'true'; })[0] || cols[0];
  var fl = parseInt(txt(e, 'FixedLeft'), 10) || 0, fr = parseInt(txt(e, 'FixedRight'), 10) || 0;
  var sbH = showSb ? 16 : 0, rowH = 19;
  var rows = des && c.h >= 40 ? 1 : 0;
  var sel = rows && (c.mainList || cellMode || !!selBack || !!selText);
  var total = cols.reduce(function (s, x) { return s + x.w; }, 0), inner = Math.max(1, c.w - 2);
  var cls = (txt(e, 'VerticalLines') === 'false' ? '' : ' vl') + (txt(e, 'HorizontalLines') === 'false' ? ' no-hl' : '') + tfs.cls;
  var st = (hn > 1 || hdrOff ? ';--head-h:' + headH + 'px' : '') + colorStyle('--alt-bg', altBack) + colorStyle('background', colorCss(e, 'BackColor')) +
    colorStyle('border-color', colorCss(e, 'BorderColor')) + tfs.st;
  function fx(col, w) { return 'flex:0 0 ' + w + 'px;width:' + w + 'px'; }
  function fixedCls(i) { return (i < fl || i >= cols.length - fr) && cols.length ? ' fixed' : ''; }
  function hid(col) { return col.visible ? '' : ';opacity:.35'; }
  var heads = cols.map(function (col, i) {
    var ce = col.e, cfs = fontSpec(ce, 'HeaderFont'), fs = cfs.cls || cfs.st ? cfs : hfs, cap = T(col.headers, R.lang), pic = hasPicture(ce, 'HeaderPicture');
    var bg = colorCss(ce, 'HeaderBackColor') || hBack, fg = txt(ce, 'Enabled') === 'false' ? 'var(--text-dis)' : colorCss(ce, 'HeaderTextColor') || hCol;
    var ha = alignCls(ce, 'HeaderHorizontalAlign', 'ha') + (pic && !cap ? ' ha-Center' : '');
    return '<span class="colh' + fs.cls + ha + fixedCls(i) + '" id="colh' + col.colk + '" data-col="' + col.colk + '" style="' + esc(fx(col, col.w) + colorStyle('background', bg) + colorStyle('color', fg) + fs.st + hid(col)) +
      '" data-k="' + c.k + '" data-i="' + c.cols.indexOf(col) + '" title="' + esc((T(col.tips, R.lang) || cap || col.name) + ' [' + col.name + '] — клик для выбора колонки') + '">' +
      (pic ? pic16(col.name) : '') + (col.w >= 20 ? esc(cap) : '') + '</span>';
  }).join('');
  var h = hdrOff ? '' : '<div class="th' + (hn > 1 ? ' mh' : '') + '"' + styleAttr((hn > 1 ? 'height:' + headH + 'px' : '') + colorStyle('background', hBack)) + '>' + heads + '</div>';
  function sample(col, rowSel) {
    var ce = col.e, ed = col.editor || txt(ce, 'Editor'), vt = col.vt, num = false, ctr = false, html = '', ro = c.pr.ro || col.ro, extra = '';
    var ha = txt(ce, 'HorizontalAlignInColumn'), isNum = !!vt && vt.kind === 'Number' && !vt.composite;
    var tcol = tree && col === treeCol;
    if (tcol) html += ICON.treePlus + ICON.folder;
    if (c.pr.ro) { /* empty sample */ }
    else if (hasPicture(ce, 'RowsPictures') || col.name === 'Картинка') { html += ICON.item; ctr = true; }
    else if (ed === 'CheckBox') { html += '<span class="box' + (txt(ce, 'CheckBoxThreeState') === 'true' ? ' mix' : '') + '"></span>'; ctr = true; }
    else if (isNum) { html += (txt(ce, 'Hyperlink') === 'true' ? '<span class="lnk">' : '') + numText(parseInt(vt.fd, 10) || 0) + (txt(ce, 'Hyperlink') === 'true' ? '</span>' : ''); num = true; }
    else if (col.ro) { /* empty sample */ }
    else if (ed === 'ChoiceField') html += '<span class="abl">ab|</span>' + (txt(ce, 'Hyperlink') === 'true' ? '<span class="lnk">&lt;Поле выбора&gt;</span>' : '&lt;Поле выбора&gt;');
    else if (!ed || ed === 'InputField') html += '<span class="abl">ab|</span>' + (txt(ce, 'Hyperlink') === 'true' ? '<span class="lnk">&lt;Поле ввода&gt;</span>' : '&lt;Поле ввода&gt;');
    if (ha === 'Right') { num = true; ctr = false; } else if (ha === 'Center') { ctr = true; num = false; }
    return { html: html, num: num, ctr: ctr, ro: ro && !html };
  }
  function row(idx, isSel, isAlt) {
    var firstEdit = cols.filter(function (x) { return !x.ro && !c.pr.ro; })[0];
    var rst = 'top:calc(var(--head-h) + ' + idx * rowH + 'px)' + (!isSel && !isAlt ? colorStyle('background', fldBack) : '');
    if (isSel && !cellMode) rst += colorStyle('background', selBack) + colorStyle('color', selText);
    return '<div class="tr' + (isSel && !cellMode ? ' rsel' : '') + (isAlt ? ' alt' : '') + '"' + (idx === 0 ? ' id="row' + c.k + '" data-row="' + c.k + '" title="Строка табличного поля ' + esc(c.name) + '"' : '') + ' style="' + esc(rst) + '">' +
      cols.map(function (col, i) {
        var s = sample(col, isSel), ce = col.e, cfs = fontSpec(ce, 'TextFont'), cs = fx(col, col.w);
        var csel = isSel && cellMode && col === firstEdit;
        var fg = txt(ce, 'Enabled') === 'false' ? 'var(--text-dis)' : colorCss(ce, 'FieldTextColor') || fldText || tText;
        if (csel) cs += colorStyle('background', selBack) + colorStyle('color', selText);
        else { if (!(isSel && !cellMode)) cs += colorStyle('background', colorCss(ce, 'FieldBackColor')); if (!(isSel && !cellMode && selText)) cs += colorStyle('color', fg); }
        return '<span class="cell' + (s.ro ? ' ro' : '') + (s.num ? ' num' : '') + (s.ctr ? ' ctr' : '') + fixedCls(i) + (csel ? ' csel' : '') + cfs.cls + '" id="colc' + (idx === 0 ? col.colk : 'a' + col.colk) + '" style="' + esc(cs + cfs.st + hid(col)) + '">' + s.html + '</span>';
      }).join('') + '</div>';
  }
  if (rows) {
    h += row(0, sel, false);
    if (alt && c.h >= headH + 2 * rowH + sbH + 2) h += row(1, false, true);
  }
  if (footer) {
    h += '<div class="tf" style="' + esc('bottom:' + sbH + 'px;height:' + footH + 'px' + colorStyle('background', fBack)) + '">' + cols.map(function (col, i) {
      var cfs = fontSpec(col.e, 'FooterFont'), fs = cfs.cls || cfs.st ? cfs : ffs;
      var bg = colorCss(col.e, 'FooterBackColor') || fBack, fg = txt(col.e, 'Enabled') === 'false' ? 'var(--text-dis)' : colorCss(col.e, 'FooterTextColor') || fCol;
      return '<span class="fd' + fs.cls + fixedCls(i) + '" style="' + esc(fx(col, col.w) + colorStyle('background', bg) + colorStyle('color', fg) + fs.st + hid(col)) + '">' + (col.w >= 20 ? esc(T(col.footers, R.lang)) : '') + '</span>';
    }).join('') + '</div>';
  }
  var pct = total > inner ? Math.max(8, Math.round(100 * inner / total)) : 100;
  if (showSb) h += '<div class="sb"' + (showVsb ? ' style="right:16px"' : '') + '><span class="sbb">' + ICON.arrowLeft + '</span><span class="sbt" style="flex:1"><i style="width:' + pct + '%"></i></span><span class="sbb">' + ICON.arrowRight + '</span></div>';
  if (showVsb) h += '<div class="vsb"' + (showSb ? ' style="bottom:16px"' : '') + '><span class="sbb">' + ICON.arrowUp + '</span><span class="sbb">' + ICON.arrowDown + '</span></div>';
  return { cls: cls, st: st, inner: h };
}

function btns(e) {
  return kids(kid(e, 'Buttons'), 'Button').map(function (b) {
    return { name: b.getAttribute('name') || '', id: b.getAttribute('id') || '', kind: b.getAttribute('kind') || 'Action', checked: b.getAttribute('checked') === 'true',
      action: b.getAttribute('action') || '', actionKind: b.getAttribute('actionKind') || '', rep: b.getAttribute('representation') || 'Auto', pic: b.getAttribute('hasPicture') === 'true',
      caps: loc(b, 'Caption'), tips: loc(b, 'Tooltip'), kids: btns(b) };
  });
}
function btnIcon(b, cap) {
  if (b.icoKey && ICON[b.icoKey]) return ICON[b.icoKey];
  if (STD_ICON[b.action]) return ICON[STD_ICON[b.action]];
  if (b.actionKind === 'OpenMovements') return ICON.movements;
  var s = (b.name + ' ' + cap).toLowerCase();
  if (s.indexOf('провод') >= 0 || s.indexOf('результат') >= 0 || s.indexOf('бухгалтер') >= 0) return ICON.movements;
  if (s.indexOf('справка') >= 0) return ICON.help;
  return ICON.picture;
}
// x: shared CommandBar look {dis, fs, col, back}; applied to every button of the bar (render-spec §13, §14)
function cbtnStyle(x, b) {
  return styleAttr(colorStyle('background', x.back) + colorStyle('color', x.col));
}
function cbtn(b, R, x) {
  x = x || { fs: { cls: '', st: '' } };
  if (b.kind === 'Separator') return '<span class="csep" data-b="' + (b.bk !== undefined ? b.bk : '') + '" title="Разделитель: ' + esc(b.name) + '"></span>';
  var cap = T(b.caps, R.lang);
  if (!cap && b.kind === 'Submenu') {
    if (b.name === 'Подменю') cap = 'Действия';
    else if (b.name === 'Подменю1' || b.name === 'Подменю2') cap = 'Перейти';
  }
  if (!cap) cap = b.name;
  var tip = T(b.tips, R.lang) || cap || b.name;
  var stdIco = STD_ICON[b.action], h = '';
  var picture = !!stdIco || b.rep === 'Picture' || b.rep === 'PictureAndText' || (b.rep === 'Auto' && b.pic);
  var text = !stdIco && (b.rep === 'Text' || b.rep === 'PictureAndText' || (b.rep === 'Auto' && !b.pic));
  if (picture) h += '<span class="ic">' + btnIcon(b, cap) + '</span>';
  if (text) h += '<span class="cap' + x.fs.cls + '"' + styleAttr(x.fs.st) + '>' + esc(cap) + '</span>';
  if (b.kind === 'Submenu') h += '<span class="ar">' + ICON.drop + '</span>';
  if (!h) return '';
  var ok = b.action === 'Standard:141' || b.def || /(ОК|OK)$/i.test(b.name);
  var bAttr = b.bk !== undefined ? ' id="b' + b.bk + '" data-b="' + b.bk + '"' : '';
  return '<span class="cbtn' + (ok ? ' ok' : '') + (b.checked ? ' on' : '') + (x.dis || b.dis ? ' dis' : '') + '"' + bAttr + cbtnStyle(x, b) + ' title="' + esc(tip + (b.action ? ' [' + b.action + ']' : '')) + '">' + h + '</span>';
}
function autoItems(list, x) {
  x = x || { fs: { cls: '', st: '' } };
  return (list || []).map(function (b) {
    if (b.sep) return '<span class="csep"></span>';
    var at = ' id="b' + b.bk + '" data-b="' + b.bk + '"' + cbtnStyle(x, b) + ' title="' + esc((T(b.caps, 'ru') || b.name) + ' [автозаполнение]') + '"';
    var cc = 'cbtn' + (b.okBtn ? ' ok' : '') + (x.dis || b.dis ? ' dis' : '');
    if (b.textSubmenu) return '<span class="' + cc + '"' + at + '><span class="cap' + x.fs.cls + '"' + styleAttr(x.fs.st) + '>' + esc(T(b.caps, 'ru') || b.name) + '</span><span class="ar">' + ICON.drop + '</span></span>';
    if (b.textBtn) return '<span class="' + cc + '"' + at + '><span class="cap' + x.fs.cls + '"' + styleAttr(x.fs.st) + '>' + esc(T(b.caps, 'ru') || b.name) + '</span></span>';
    return '<span class="' + cc + '"' + at + '><span class="ic">' + ICON[b.icoKey] + '</span></span>';
  }).join('');
}
function commandBar(c, R) {
  var e = c.e, p = c.pr, auto = txt(e, 'AutoFill') === 'true', par = c.parent >= 0 && R && R.m ? R.m.C[c.parent] : null;
  var formBar = c.parent < 0 || !!(par && par.tag === 'Page' && par.parent < 0 && c.w >= par.w - 4 && (c.t <= 4 || c.t + c.h >= par.h - 4));
  var raw = (R && R.m && R.m.btnsOf && R.m.btnsOf[c.k]) ? R.m.btnsOf[c.k].slice() : btns(e);
  if (auto && !formBar) {
    raw = raw.filter(function (b) {
      if (STD_TABLE_SKIP[b.action]) return false;
      var ru = T(b.caps, 'ru') || '';
      if (ru.indexOf('История отбор') >= 0 || ru.indexOf('Сохранить видимость') >= 0) return false;
      return true;
    });
  }
  var x = { dis: !p.en, fs: fontSpec(e, 'Font'), col: colorCss(e, 'ButtonTextColor'), back: colorCss(e, 'ButtonBackColor') };
  var own = raw.map(function (b) { return cbtn(b, R, x); }).join('');
  var autoList = (R && R.m && R.m.autoBtnsOf && R.m.autoBtnsOf[c.k]) || [];
  var a = auto ? autoItems(autoList, x) : '';
  var dots = R.mode === 'designer' ? '<span class="cbtn"><span class="ic">' + ICON.cfgMarker + '</span></span>' : '';
  var ph = par ? par.h : R.m.height, ha = txt(e, 'HorizontalAlign'), right = ha === 'Right';
  var cls = (c.t + c.h / 2 > ph / 2 && formBar ? ' bottom' : '') + (right ? ' right' : '') + (ha === 'Center' ? ' center' : '') + (c.h > 32 ? ' tall' : '') +
    (txt(e, 'Orientation') === 'Vertical' ? ' vert' : '') + (p.tr ? ' tr-bg' : '') + (p.bd ? ' bd-' + p.bd : '');
  var inner = formBar ? (right ? dots + own + (own ? '' : a) : (own ? own + (a ? '<span class="csep"></span>' + a : '') : a) + dots) : a + own + dots;
  return { cls: cls, st: colorStyle('background', colorCss(e, 'BackColor')) + colorStyle('border-color', colorCss(e, 'BorderColor')), inner: inner };
}

// Panel tabs (render-spec §10, §11): the strip position follows the geometry of the first page (the ShowTabs codes of the corpus do not follow the
// platform enum order), ShowTabs=0 means no tabs; tabs are never squeezed, the ones that do not fit are cut by the strip and arrows are drawn.
function tabStrip(pw, ph, pg, st) {
  if (st === '0') return { mode: 'none', size: 0 };
  if (pg.t > 4 && pg.t >= pg.l) return { mode: 'top', size: pg.t };
  if (pg.l > 4) return { mode: 'left', size: pg.l };
  if (st) {
    var bh = ph - (pg.t + pg.h), rw = pw - (pg.l + pg.w);
    if (bh > 4 && bh >= rw) return { mode: 'bottom', size: bh };
    if (rw > 4) return { mode: 'right', size: rw };
  }
  return { mode: 'none', size: 0 };
}
function panelStrip(panel, pages, R) {
  return tabStrip(panel.k >= 0 ? panel.w : R.m.width, panel.k >= 0 ? panel.h : R.m.height, pages[0], panel.k >= 0 ? panel.showTabs : R.m.showTabs);
}
function tabsHtml(panel, pages, first, R) {
  var ts = panelStrip(panel, pages, R), e = panel.e, pfs = fontSpec(e, 'Font'), tc = colorCss(e, 'TextColor'), bc = colorCss(e, 'BorderColor');
  var pdis = !!(panel.pr && !panel.pr.en), cls, st, total = 0, pw = panel.k >= 0 ? panel.w : R.m.width;
  if (ts.mode === 'none') return '';
  var multi = ts.mode === 'top' && ts.size >= 34;
  if (ts.mode === 'top') { cls = 'ptabs' + (multi ? ' multi' : ''); st = 'height:' + ts.size + 'px'; }
  else if (ts.mode === 'bottom') { cls = 'ptabs btabs'; st = 'height:' + ts.size + 'px'; }
  else if (ts.mode === 'left') { cls = 'ptabs vtabs'; st = 'width:' + ts.size + 'px'; }
  else { cls = 'ptabs vtabs rtabs'; st = 'width:' + ts.size + 'px'; }
  var html = pages.map(function (p) {
    var cap = T(p.caps, R.lang) || p.name, ti = hasPicture(p.e, 'TitlePicture');
    total += textW(cap, (pfs.bold ? 'bold ' : '') + '8pt Tahoma, Arial, sans-serif') + 16 + 2 + 2 + (ti ? 19 : 0);
    var on = p.k === first, sty = 'flex:none' + colorStyle('color', tc) + (on ? colorStyle('border-color', bc) : '') + pfs.st;
    return '<span class="ptab' + (on ? ' on' : '') + (p.visible ? '' : ' hid') + (pdis || (p.pr && !p.pr.en) ? ' dis' : '') + pfs.cls + '" style="' + esc(sty) + '" data-k="' + p.k + '" data-p="' + panel.k + '">' +
      (ti ? '<span class="pic16">' + ICON.picture + '</span>' : '') + esc(cap) + '</span>';
  }).join('');
  var scroll = (ts.mode === 'top' || ts.mode === 'bottom') && !multi && total > pw ? '<span class="pscroll"><i class="off">' + ICON.arrowLeft + '</i><i>' + ICON.tabRight + '</i></span>' : '';
  return '<div class="' + cls + '" style="' + st + '">' + html + scroll + '</div>';
}
// pages of a panel (also of the pseudo panel of the form root): panel colours / border / transparency go to every page
function pagesHtml(panel, seq, first, R) {
  var e = panel.e, pr = panel.pr || {};
  var pcls = (pr.tr ? ' tr-bg' : '') + (pr.bd ? ' bd-' + pr.bd : ''), pst = colorStyle('background', colorCss(e, 'BackColor')) + colorStyle('border-color', colorCss(e, 'BorderColor'));
  return seq.map(function (p) {
    return '<div class="c t-Page' + (p.k === first ? ' on' : '') + pcls + '" id="c' + p.k + '" data-k="' + p.k + '" data-p="' + panel.k + '" style="' + pos(p) + pst + '" title="' + esc(p.name) + '">' +
      ordered(R.m.kidsOf[p.k]).filter(function (x) { return vis(x, R); }).map(function (x) { return ctl(x, R); }).join('') + '</div>';
  }).join('');
}

// CalendarField (render-spec §17)
function calendarBox(c, R) {
  var e = c.e, mode = txt(e, 'SelectionMode'), fs = fontSpec(e, 'Font'), small = c.w < 140 || c.h < 110;
  var days = ['Пн', 'Вт', 'Ср', 'Чт', 'Пт', '<b class="wk">Сб</b>', '<b class="wk">Вс</b>'].map(function (d) { return '<span>' + d + '</span>'; }).join('');
  var curIdx = mode === 'Multiple' ? { 17: 'cur', 24: 'cur' } : mode === 'Interval' ? { 23: 'cur', 24: 'rng', 25: 'cur' } : { 24: 'cur' };
  var nums = [28, 29, 30, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 1].map(function (n, i) {
    return '<span class="' + (curIdx[i] || (i < 3 || i >= 34 ? 'mut' : (i % 7 >= 5 ? 'wk' : ''))) + '">' + n + '</span>';
  }).join('');
  var back = colorCss(e, 'ButtonBackColor'), txtc = colorCss(e, 'FieldTextColor');
  var st = colorStyle('background', colorCss(e, 'FieldBackColor')) + colorStyle('--holiday', colorCss(e, 'HolidayColor')) + colorStyle('--sel-bg', colorCss(e, 'SelectionBackColor')) +
    colorStyle('--sel-text', colorCss(e, 'SelectionTextColor')) + colorStyle('border-color', colorCss(e, 'BorderColor')) + fs.st;
  return {
    cls: fs.cls + (txt(e, 'CalendarNavigation') === 'false' ? ' nonav' : '') + (txt(e, 'ShowCurrentDate') === 'false' || small ? ' nocur' : '') + (c.pr.tr ? ' tr-bg' : '') + (c.pr.bd ? ' bd-' + c.pr.bd : ''),
    st: st,
    inner: '<div class="cal-hd"' + styleAttr(colorStyle('background', back)) + '><span class="nav">◂◄</span><b>Октябрь 2026</b><span class="nav">►▸</span></div>' +
      '<div class="cal-wk">' + days + '</div><div class="cal-gr"' + styleAttr(colorStyle('color', txtc)) + '>' + nums + '</div><div class="cal-ft">22 октября 2026 г.</div>'
  };
}
// scroll bars of SpreadsheetDocumentField / TextDocumentField (render-spec §16, §23)
function sdocBars(c, vBar, hBar) {
  var v = vBar ? '<div class="sdoc-v"' + (hBar ? '' : ' style="bottom:0"') + '><span class="sbb">' + ICON.arrowUp + '</span><span class="sbb">' + ICON.arrowDown + '</span></div>' : '';
  var hb = hBar ? '<div class="sdoc-h"' + (vBar ? '' : ' style="right:0"') + '><span class="sbb">' + ICON.arrowLeft + '</span><span class="sbb">' + ICON.arrowRight + '</span></div>' : '';
  return v + hb + (vBar && hBar ? '<span class="sdoc-c"></span>' : '');
}
function sdocBox(c, R) {
  var e = c.e, hs = txt(e, 'HorizontalScrollBar'), vs = txt(e, 'VerticalScrollBar'), tall = c.h >= 40;
  var grp = txt(e, 'ShowGroups') !== 'false', hd = txt(e, 'ShowHeaders') === 'true', top = grp ? 9 : 0;
  var inner = (grp ? '<div class="shd-g"></div>' : '') +
    (hd ? '<div class="shd-h"' + (grp ? ' style="top:9px"' : '') + '></div><div class="shd-v"' + (grp ? ' style="top:9px"' : '') + '></div>' : '') +
    sdocBars(c, tall && vs !== 'DontUse' && vs !== 'false', tall && hs !== 'DontUse' && hs !== 'false');
  return { cls: (txt(e, 'ShowGrid') === 'false' ? '' : ' grid') + (c.pr.bd ? ' bd-' + c.pr.bd : ''), st: colorStyle('border-color', colorCss(e, 'BorderColor')), inner: inner };
}
function textDocBox(c, R) {
  var e = c.e, fs = fontSpec(e, 'Font'), ph = R.mode === 'designer' ? '<span class="txt-ph' + fs.cls + '"' + styleAttr(colorStyle('color', colorCss(e, 'TextColor')) + fs.st) + '>Текст</span>' : '';
  return { cls: (c.pr.bd ? ' bd-' + c.pr.bd : ''), st: colorStyle('background', colorCss(e, 'BackColor')) + colorStyle('border-color', colorCss(e, 'BorderColor')),
    inner: ph + (c.h >= 40 ? sdocBars(c, true, false) : '') };
}
// ProgressBar (render-spec §19)
function progressBox(c, R) {
  var e = c.e, fs = fontSpec(e, 'Font'), ss = txt(e, 'ShowStyle'), vert = txt(e, 'Orientation') === 'Vertical';
  return {
    cls: (ss === 'Broken' || ss === 'BrokenTilt' ? ' ps-' + ss : '') + (vert ? ' vert' : '') + (c.pr.bd ? ' bd-' + c.pr.bd : ''),
    st: colorStyle('background', colorCss(e, 'BackColor')) + colorStyle('border-color', colorCss(e, 'BorderColor')),
    inner: (c.h >= 8 ? '<div class="prg-fill"></div>' : '') + (txt(e, 'ShowPercent') === 'true' && c.h >= 8 ? '<span class="prg-pct' + fs.cls + '"' + styleAttr(fs.st) + '>38%</span>' : '')
  };
}
// TrackBar (render-spec §20)
function trackBox(c, R) {
  var e = c.e, mn = parseFloat(txt(e, 'MinValue')) || 0, mx = txt(e, 'MaxValue') === '' ? 10 : parseFloat(txt(e, 'MaxValue')), step = parseFloat(txt(e, 'MarkingStep')) || 1;
  var ma = txt(e, 'MarkingAppearance'), n = Math.max(1, Math.min(21, Math.floor((mx - mn) / step) + 1)), i, ticks = '';
  var tcol = colorCss(e, 'TextColor'), tst = styleAttr(colorStyle('background', tcol));
  for (i = 0; i < n; i++) ticks += '<i' + tst + '></i>';
  var top = ma === 'TopLeft' || ma === 'Both', bot = ma !== 'DontShow' && ma !== 'TopLeft';
  return {
    cls: (txt(e, 'Orientation') === 'Vertical' ? ' vert' : '') + (c.pr.bd ? ' bd-' + c.pr.bd : ''),
    st: colorStyle('background', colorCss(e, 'BackColor')),
    inner: (top ? '<div class="trk-ticks top">' + ticks + '</div>' : '') + '<div class="trk-line"' + styleAttr(colorStyle('border-color', colorCss(e, 'BorderColor'))) + '><span class="trk-thumb"' +
      styleAttr(colorStyle('background', colorCss(e, 'ButtonBackColor'))) + '></span></div>' + (bot ? '<div class="trk-ticks">' + ticks + '</div>' : '')
  };
}
// ListBox (render-spec §21): three sample rows in the designer view, the first one selected
function listBox(c, R) {
  var e = c.e, rows = '', n = R.mode === 'designer' ? Math.min(3, Math.floor((c.h - 2) / 17)) : 0, i, fg = colorCss(e, 'FieldTextColor');
  for (i = 0; i < n; i++) {
    rows += '<div class="lb-row' + (i === 0 ? ' sel' : '') + '" style="top:' + (i * 17) + 'px' + (i ? colorStyle('color', fg) : '') + '">' + (txt(e, 'ShowCheckBox') === 'true' ? '<span class="box"></span>' : '') +
      (txt(e, 'ShowPicture') === 'true' ? ICON.item : '') + '<span class="cap">Элемент списка ' + (i + 1) + '</span></div>';
  }
  return { cls: (c.pr.bd ? ' bd-' + c.pr.bd : ''), st: colorStyle('background', colorCss(e, 'FieldBackColor')) + colorStyle('border-color', colorCss(e, 'BorderColor')), inner: rows };
}
// Splitter (render-spec §22)
function splitterBox(c, R) {
  var e = c.e, o = txt(e, 'Orientation'), back = colorCss(e, 'BackColor');
  if (o === 'Auto' || (c.w > 8 && c.h > 8)) o = c.w < c.h ? 'Vertical' : 'Horizontal';
  return { cls: (o === 'Horizontal' ? ' hor' : '') + (back ? ' colored' : '') + (Math.max(c.w, c.h) < 20 ? ' nh' : '') + (c.pr.bd ? ' bd-' + c.pr.bd : ''), st: colorStyle('--spl-bg', back), inner: '' };
}
// PictureDecoration without a label (render-spec §15): an empty frame, or the picture stub when <Picture> is set
function pictureBox(c, R) {
  var e = c.e, has = hasPicture(e), np = T(loc(e, 'NonselectedPictureText'), R.lang), ps = txt(e, 'PictureSize');
  return {
    cls: ' pic-box' + (has ? ' pic-has' : '') + (ps === 'AutoSize' || ps === 'RealSize' || ps === 'Tile' ? ' ps-tl' : '') + (c.pr.tr ? ' tr-bg' : '') + (c.pr.bd ? ' bd-' + c.pr.bd : ''),
    st: colorStyle('background', colorCss(e, 'BackColor')) + colorStyle('border-color', colorCss(e, 'BorderColor')),
    inner: !has && np ? '<span class="nptxt">' + esc(np) + '</span>' : ''
  };
}
function ctl(c, R) {
  var tag = c.tag, k = c.k, des = R.mode === 'designer', p = c.pr;
  var cls = 'c t-' + esc(tag) + (c.visible ? '' : ' hid') + (!p.en && tag !== 'UsualGroup' && tag !== 'Panel' ? ' dis' : ''), st = pos(c), inner = '';
  var cap = esc(T(c.caps, R.lang)), capspan = '<span class="cap" data-k="' + k + '">' + cap + '</span>';
  var ch = ordered(R.m.kidsOf[k]).filter(function (x) { return vis(x, R); });
  var body = '';
  if (tag === 'PictureDecoration' && !cap && c.w > 24 && c.h > 24) {
    var pb = pictureBox(c, R); cls += pb.cls; st += pb.st; inner = pb.inner;
  } else if (tag === 'LabelDecoration' || tag === 'PictureDecoration') {
    var l = label(c, R, cls, st); cls = l.cls; st = l.style; inner = l.inner;
  } else if (tag === 'Button') {
    var bb = buttonBox(c, R); cls += bb.cls; st += bb.st; inner = bb.inner;
  } else if (tag === 'CommandBar') {
    var cb = commandBar(c, R); cls += cb.cls; st += cb.st; inner = cb.inner;
  } else if (tag === 'InputField' || tag === 'ChoiceField') {
    var fl = inputField(c, R); cls += fl.cls; st += fl.st; inner = fl.inner;
  } else if (tag === 'CheckBox' || tag === 'RadioButton') {
    var cx = checkBox(c, R); cls += cx.cls; st += cx.st; inner = cx.inner;
  } else if (tag === 'Table') {
    var tb = tableBox(c, R); cls += tb.cls + (c.pr.bd ? ' bd-' + c.pr.bd : '') + (c.pr.tr ? ' tr-bg' : ''); st += tb.st; inner = tb.inner;
  } else if (tag === 'UsualGroup') {
    var g = groupBox(c, R, T(c.caps, R.lang)); cls += g.cls; st += g.st; inner = g.inner;
  } else if (tag === 'CalendarField') {
    var cal = calendarBox(c, R); cls += cal.cls; st += cal.st; inner = cal.inner;
  } else if (tag === 'TrackBar') {
    var tk = trackBox(c, R); cls += tk.cls; st += tk.st; inner = tk.inner;
  } else if (tag === 'ProgressBar') {
    var pg = progressBox(c, R); cls += pg.cls; st += pg.st; inner = pg.inner;
  } else if (tag === 'SpreadsheetDocumentField') {
    var sd = sdocBox(c, R); cls += sd.cls; st += sd.st; inner = sd.inner;
  } else if (tag === 'TextDocumentField') {
    var td = textDocBox(c, R); cls += td.cls; st += td.st; inner = td.inner;
  } else if (tag === 'ListBox') {
    var lb = listBox(c, R); cls += lb.cls; st += lb.st; inner = lb.inner;
  } else if (tag === 'HTMLDocumentField') {
    cls += c.pr.bd ? ' bd-' + c.pr.bd : ''; st += colorStyle('border-color', colorCss(c.e, 'BorderColor')); inner = des ? '<span class="html-ph">HTML</span>' : '';
  } else if (tag === 'GraphicalSchemaField' || tag === 'GeographicalSchemaField' || tag === 'PDFDocumentField') {
    cls += (tag === 'GraphicalSchemaField' ? ' gs-grid' : '') + (c.pr.tr ? ' tr-bg' : '') + (c.pr.bd ? ' bd-' + c.pr.bd : '');
    st += colorStyle('background-color', colorCss(c.e, 'BackColor')) + colorStyle('border-color', colorCss(c.e, 'BorderColor'));
  } else if (tag === 'Splitter') {
    var sp = splitterBox(c, R); cls += sp.cls; st += sp.st;
  } else if (tag !== 'Panel') {
    inner = '<span class="kind">' + esc(tag) + '</span>' + capspan;
  }
  var pages = tag === 'Panel' ? ch.filter(function (x) { return x.tag === 'Page'; }) : [];
  if (pages.length) {
    var seq = pages.slice().sort(function (a, b) { return a.k - b.k; });
    var first = R.active[k];
    if (!seq.some(function (p) { return p.k === first; })) { first = (seq.filter(function (p) { return p.visible; })[0] || seq[0]).k; R.active[k] = first; }
    var th = seq.length > 1 || (c.showTabs && c.showTabs !== '0') ? tabsHtml(c, seq, first, R) : '';
    if (!th) cls += ' no-tabs';
    body += th + pagesHtml(c, seq, first, R);
    ch = ch.filter(function (x) { return x.tag !== 'Page'; });
  }
  body += ch.map(function (x) { return ctl(x, R); }).join('');
  return '<div class="' + cls + '" id="c' + k + '" data-k="' + k + '" style="' + st + '" title="' + esc(titleOf(c, R)) + '">' + inner + body + '</div>';
}

// ------------------------------------------------------------------ inspector (properties with provenance)
var PROV = { bin: 'из Form.bin', style: 'стиль', auto: 'автозаполнение', def: 'по умолчанию' };
var STYLED = { LabelDecoration: 1, UsualGroup: 1, InputField: 1, CheckBox: 1, RadioButton: 1, Button: 1, CommandBar: 1 };
function inspect(c, R) {
  var e = c.e, lang = R.lang, rows = [], effVis = vis(c, R), hasOv = R && R.visOverride && (c.k in R.visOverride);
  function r(a, b, p) { rows.push('<tr><td>' + a + '</td><td>' + b + (p ? '<span class="prov ' + p + '">' + PROV[p] + '</span>' : '') + '</td></tr>'); }
  function present(t) { return !!kid(e, t); }
  function colorRow(label, tag, defv) {
    var n = kid(e, tag);
    if (!n) { r(label, esc(defv), 'def'); return; }
    var v = n.getAttribute('value') || n.getAttribute('kind') || '';
    r(label, '<span style="display:inline-block;width:10px;height:10px;border:1px solid #888;vertical-align:middle;background:' + esc(v) + '"></span> ' + esc(v) + (n.getAttribute('name') ? ' (' + esc(n.getAttribute('name')) + ')' : ''), n.getAttribute('kind') === 'style' ? 'style' : 'bin');
  }
  r('Tag', '&lt;' + esc(c.tag) + '&gt;'); r('name', esc(c.name)); r('id', esc(c.id));
  r('Left / Top', 'local ' + c.l + ', ' + c.t + ' · abs ' + c.al + ', ' + c.at, 'bin');
  r('Width / Height', c.w + ' × ' + c.h, 'bin');
  r('ZOrder', String(c.z), present('ZOrder') ? 'bin' : 'def');
  if (present('TabOrder')) r('TabOrder', esc(txt(e, 'TabOrder')), 'bin');
  r('Visible', c.visible ? 'true' : 'false', present('Visible') ? 'bin' : 'def');
  var visBtn = '<button type="button" class="ofr-act" data-ofr-eye="' + c.k + '">' + (effVis ? ICON.eyeOff + ' Скрыть на форме (H)' : ICON.eye + ' Показать на форме (H)') + '</button>' +
    (hasOv ? ' <button type="button" class="ofr-act" data-ofr-unoverride="' + c.k + '">Сброс</button>' : '');
  r('Просмотр (слой)', (effVis ? 'видим' : 'скрыт') + (hasOv ? ' <i>(вручную)</i>' : '') + ' ' + visBtn);
  if (R && R.m && R.m.ov && R.m.ov[c.k] && R.m.ov[c.k].length) {
    var ovList = R.m.ov[c.k].slice().sort(function (a, b) { return (a.kind === 'exact' ? 0 : 1) - (b.kind === 'exact' ? 0 : 1); });
    r('Перекрытия (' + ovList.length + ')', ovList.map(function (o2) {
      var oc = R.m.C[o2.k], oVis = vis(oc, R);
      return '<span class="ofr-ovrow"><span class="ofr-eye' + (oVis ? '' : ' off') + '" data-ofr-eye="' + oc.k + '" title="Скрыть/показать">' + (oVis ? ICON.eye : ICON.eyeOff) + '</span>' +
        '<span class="ofr-link" data-ofr-sel="' + oc.k + '">' + esc(oc.tag) + ' <b>' + esc(oc.name) + '</b></span> ' +
        '<span class="ofr-tag">' + (o2.kind === 'exact' ? '1:1' : 'част.') + ' · ' + (o2.above ? 'сверху' : 'снизу') + (oc.visible ? '' : ' · скрыт в 1С') + '</span></span>';
    }).join('<br>'));
  }
  if (c.tag === 'UsualGroup' && R && R.m && R.m.members && R.m.members[c.k] && R.m.members[c.k].length) {
    var mems = R.m.members[c.k];
    r('В рамке (' + mems.length + ')', mems.map(function (mk) {
      var mc = R.m.C[mk];
      return '<span class="ofr-link" data-ofr-sel="' + mc.k + '">' + esc(mc.tag) + ' <b>' + esc(mc.name) + '</b></span>';
    }).join(', '));
  }
  if (c.tag === 'Panel' && R && R.m) {
    var pgs = (R.m.kidsOf[c.k] || []).filter(function (x) { return x.tag === 'Page'; });
    if (pgs.length) {
      r('Страницы (' + pgs.length + ')', pgs.map(function (pg, i) {
        var on = R.active[c.k] === pg.k, cap2 = T(pg.caps, lang) || pg.name;
        return '<span class="ofr-chip' + (on ? ' on' : '') + '" data-ofr-page="' + pg.k + '">' + (i + 1) + ': ' + esc(cap2) + (pg.visible ? '' : ' [скрыта]') + '</span>';
      }).join(' '));
    }
  }
  r('DataPath', (c.dp || c.dpName) ? 'attributeId ' + esc(c.dp) + ' → ' + esc(c.dpName) : '', (c.dp || c.dpName) ? 'bin' : '');
  r('Caption', esc(T(c.caps, lang)), Object.keys(c.caps).length ? 'bin' : '');
  if (STYLED[c.tag]) {
    var f = kid(e, 'Font');
    r('Font', f ? (f.getAttribute('bold') === 'true' ? 'bold' : 'обычный') + (f.getAttribute('face') ? ', ' + esc(f.getAttribute('face')) : '') + (f.getAttribute('size') ? ' ' + esc(f.getAttribute('size')) : '') : 'Tahoma 8, обычный', f ? (f.getAttribute('source') === 'style' ? 'style' : 'bin') : 'def');
    colorRow('TextColor', 'TextColor', '#000000');
    colorRow('BackColor', 'BackColor', c.tag === 'LabelDecoration' ? '#FCFAEB (фон формы)' : 'авто');
  }
  if (c.tag === 'LabelDecoration' || c.tag === 'CommandBar') r('HorizontalAlign', esc(txt(e, 'HorizontalAlign') || 'Left'), present('HorizontalAlign') ? 'bin' : 'def');
  if (c.tag === 'LabelDecoration') { r('Hyperlink', txt(e, 'Hyperlink') || 'false', present('Hyperlink') ? 'bin' : 'def'); r('Picture', txt(e, 'Picture') || 'false', present('Picture') ? 'bin' : 'def'); }
  if (c.tag === 'UsualGroup') r('BorderStyle', esc(txt(e, 'BorderStyle') || (c.h > 20 ? 'Frame' : 'SingleLine')), present('BorderStyle') ? 'bin' : 'def');
  if (c.tag === 'InputField') {
    var vt = vtype(e);
    r('ValueType', vt ? esc(vt.kind) + (vt.composite ? ' (составной)' : '') + (vt.fd !== null ? ', дробных: ' + esc(vt.fd) : '') : '', vt ? 'bin' : '');
    ['ChoiceButton', 'ClearButton', 'OpenButton', 'AutoMarkIncomplete'].forEach(function (t) { r(t, txt(e, t) || (t === 'ChoiceButton' ? 'авто (...)' : 'false'), present(t) ? 'bin' : 'def'); });
  }
  if (c.tag === 'ChoiceField' && c.choiceList && c.choiceList.length) {
    r('ChoiceList (' + c.choiceList.length + ')', c.choiceList.map(function (ci) {
      var pr = T(ci.pres, lang) || ci.value;
      return '<code>' + esc(ci.value) + '</code> — ' + esc(pr);
    }).join('<br>'), 'bin');
  }
  if (c.tag === 'CommandBar') {
    var bs = (R && R.m && R.m.btnsOf && R.m.btnsOf[c.k]) || btns(e), af = txt(e, 'AutoFill') === 'true';
    var autoBs = (R && R.m && R.m.autoBtnsOf && R.m.autoBtnsOf[c.k]) ? R.m.autoBtnsOf[c.k].filter(function (x) { return !x.sep; }) : [];
    r('AutoFill', af ? 'true' : 'false', present('AutoFill') ? 'bin' : 'def');
    if (af) r('Стандартные кнопки', autoBs.length ? autoBs.map(function (b) {
      return '<span class="ofr-link" data-ofr-btn="' + b.bk + '">' + esc(b.name) + '</span>';
    }).join(', ') : (c.parent < 0 ? AUTO_FORM : AUTO_TABLE).filter(function (x) { return x !== '|'; }).length + ' шт.', 'auto');
    r('Buttons (' + bs.length + ')', bs.map(function (b) {
      var cap2 = T(b.caps, lang) || b.name, subN = (b.kids || []).length;
      var act = b.actionKind === 'Handler' && b.action ? ' → <span class="ev" data-h="' + esc(b.action) + '">' + esc(b.action) + '()</span>' : (b.action ? ' <span class="ofr-tag">[' + esc(b.action) + ']</span>' : '');
      var link = b.bk !== undefined ? '<span class="ofr-link" data-ofr-btn="' + b.bk + '"><b>' + esc(b.name) + '</b></span>' : esc(b.name);
      return link + (cap2 && cap2 !== b.name ? ' («' + esc(cap2) + '»)' : '') + (b.kind !== 'Action' ? ' [' + esc(b.kind) + (subN ? ': ' + subN : '') + ']' : '') + act;
    }).join('<br>'), bs.length ? 'bin' : '');
  }
  if (c.tag === 'Table') {
    r('Footer', txt(e, 'Footer') || 'false', present('Footer') ? 'bin' : 'def');
    r('Строка данных', '<span class="ofr-link" data-ofr-row="' + c.k + '"><b>Строка табличного поля</b> (' + c.cols.length + ' кол.)</span>');
  }
  if (c.cols.length) r('Columns (' + c.cols.length + ')', c.cols.map(function (x) {
    var hdr = T(x.headers, lang);
    var evHtml = (x.events && x.events.length) ? ' · ' + x.events.map(function (ev2) {
      return '<span class="ev" data-h="' + esc(ev2.handler) + '">' + esc(ev2.handler) + '()</span>';
    }).join(', ') : '';
    return '<span class="ofr-link" data-ofr-col="' + x.colk + '"><b>' + esc(x.name) + '</b></span>' +
      (hdr && hdr !== x.name ? ' («' + esc(hdr) + '»)' : '') +
      (x.dpName ? ' → <code>' + esc(x.dpName) + '</code>' : '') +
      ' · ' + x.w + 'px' + (x.visible ? '' : ' · скрыта') + (x.ro ? ' · только чтение' : '') + evHtml;
  }).join('<br>'), 'bin');
  r('Events', evs(c.events), c.events.length ? 'bin' : '');
  return '<table>' + rows.join('') + '</table><pre class="hljs">' + hl(c.xml, 'xml') + '</pre>';
}
function inspectCol(col, R) {
  var lang = R.lang, rows = [], tbl = R.m.C[col.ck];
  function r(a, v, p) { rows.push('<tr><td>' + a + '</td><td>' + v + (p ? '<span class="prov ' + p + '">' + PROV[p] + '</span>' : '') + '</td></tr>'); }
  r('Tag', '&lt;Column&gt; (колонка табличного поля)', 'bin');
  r('name', '<b>' + esc(col.name) + '</b>', 'bin');
  if (col.id !== '') r('id', esc(col.id), 'bin');
  if (tbl) r('Table', '<span class="ofr-link" data-ofr-sel="' + tbl.k + '">Table <b>' + esc(tbl.name) + '</b></span>');
  if (col.dp || col.dpName) r('DataPath', (col.dp ? 'attributeId ' + esc(col.dp) + ' → ' : '') + '<b>' + esc(col.dpName) + '</b>', 'bin');
  var hdr = T(col.headers, lang), ftr = T(col.footers, lang), tip = T(col.tips, lang);
  r('Header', esc(hdr), Object.keys(col.headers).length ? 'bin' : '');
  if (ftr) r('FooterText', esc(ftr), 'bin');
  if (tip) r('Tooltip', esc(tip), 'bin');
  r('Width', col.w + 'px', 'bin');
  r('Visible', col.visible ? 'true' : 'false', col.visible ? 'def' : 'bin');
  r('ReadOnly', col.ro ? 'true' : 'false', col.ro ? 'bin' : 'def');
  if (col.vt) {
    r('ValueType', esc(col.vt.kind) + (col.vt.composite ? ' (составной)' : '') + (col.vt.fd !== null ? ', дробных: ' + esc(col.vt.fd) : ''), 'bin');
  }
  if (col.editor) r('Editor (ячейка)', esc(col.editor), 'bin');
  r('Events', evs(col.events || []), (col.events && col.events.length) ? 'bin' : '');
  return '<table>' + rows.join('') + '</table><pre class="hljs">' + hl(col.xml, 'xml') + '</pre>';
}
function inspectRow(c, R) {
  var lang = R.lang, rows = [];
  function r(a, v, p) { rows.push('<tr><td>' + a + '</td><td>' + v + (p ? '<span class="prov ' + p + '">' + PROV[p] + '</span>' : '') + '</td></tr>'); }
  r('Tag', '&lt;Row&gt; (строка табличного поля)', 'bin');
  r('Table', '<span class="ofr-link" data-ofr-sel="' + c.k + '">Table <b>' + esc(c.name) + '</b></span>');
  if (c.dp || c.dpName) r('DataPath', (c.dp ? 'attributeId ' + esc(c.dp) + ' → ' : '') + '<b>' + esc(c.dpName) + '</b>', 'bin');
  r('Ячейки строки (' + c.cols.length + ')', c.cols.map(function (x) {
    var hdr = T(x.headers, lang);
    var ed = x.ro ? 'только чтение' : (x.editor || 'InputField');
    var vt = x.vt ? ' [' + x.vt.kind + ']' : '';
    var evHtml = (x.events && x.events.length) ? ' → ' + x.events.map(function (ev2) {
      return '<span class="ev" data-h="' + esc(ev2.handler) + '">' + esc(ev2.handler) + '()</span>';
    }).join(', ') : '';
    return '<span class="ofr-link" data-ofr-col="' + x.colk + '"><b>' + esc(x.name) + '</b></span>' +
      (hdr && hdr !== x.name ? ' («' + esc(hdr) + '»)' : '') + ' · ' + esc(ed + vt) + (x.visible ? '' : ' · скрыта') + evHtml;
  }).join('<br>'), c.cols.length ? 'bin' : '');
  r('События строки / таблицы', evs(c.events || []), c.events.length ? 'bin' : '');
  var rowXml = '<Row table="' + c.name + '"' + (c.dpName ? ' dataPath="' + c.dpName + '"' : '') + '>\n' +
    c.cols.map(function (x) {
      return '  <Cell column="' + x.name + '" editor="' + (x.editor || 'InputField') + '"' +
        (x.dpName ? ' dataPath="' + x.dpName + '"' : '') +
        (x.ro ? ' readOnly="true"' : '') + (x.visible ? '' : ' visible="false"') + '/>';
    }).join('\n') + '\n</Row>';
  return '<table>' + rows.join('') + '</table><pre class="hljs">' + hl(rowXml, 'xml') + '</pre>';
}
function inspectAttr(a, R) {
  var rows = [];
  function r(k2, v, p) { rows.push('<tr><td>' + k2 + '</td><td>' + v + (p ? '<span class="prov ' + p + '">' + PROV[p] + '</span>' : '') + '</td></tr>'); }
  r('Tag', '&lt;Attribute&gt; (реквизит формы)', 'bin');
  r('name', '<b>' + esc(a.name) + '</b>', 'bin');
  r('id', esc(a.id), 'bin');
  r('Main (основной реквизит)', a.main ? 'true' : 'false', a.main ? 'bin' : 'def');
  if (a.stored) r('StoredData', 'true', 'bin');
  if (a.vt) r('ValueType', esc(a.vt.kind) + (a.vt.composite ? ' (составной)' : '') + (a.vt.fd !== null ? ', дробных: ' + esc(a.vt.fd) : ''), 'bin');
  var bound = (R.m.C || []).filter(function (c) { return c.dp === a.id; });
  if (bound.length) {
    r('Привязанные элементы (' + bound.length + ')', bound.map(function (c) {
      return '<span class="ofr-link" data-ofr-sel="' + c.k + '">' + esc(c.tag) + ' <b>' + esc(c.name) + '</b></span>' + (c.dpName && c.dpName !== a.name ? ' <span class="ofr-tag">(' + esc(c.dpName) + ')</span>' : '');
    }).join('<br>'));
  }
  return '<table>' + rows.join('') + '</table><pre class="hljs">' + hl(a.xml, 'xml') + '</pre>';
}
function inspectForm(R) {
  var m = R.m, rows = [];
  function r(k2, v, p) { rows.push('<tr><td>' + k2 + '</td><td>' + v + (p ? '<span class="prov ' + p + '">' + PROV[p] + '</span>' : '') + '</td></tr>'); }
  r('Tag', '&lt;Form&gt; (обычная форма 1С 8.2)', 'bin');
  r('name', '<b>' + esc(m.formName || V_alias(R)) + '</b>', 'bin');
  if (m.formId) r('id', esc(m.formId), 'bin');
  r('Caption', esc(T(m.caps, R.lang)), Object.keys(m.caps).length ? 'bin' : '');
  r('Width / Height', m.width + ' × ' + m.height, 'bin');
  r('Элементов / Кнопок / Колонок', m.C.length + ' / ' + (m.B ? m.B.length : 0) + ' / ' + (m.Cols ? m.Cols.length : 0));
  if (m.A && m.A.length) {
    r('Attributes (' + m.A.length + ')', m.A.map(function (a) {
      return '<span class="ofr-link" data-ofr-attr="' + a.ak + '"><b>' + esc(a.name) + '</b></span>' +
        (a.vt ? ' [' + esc(a.vt.kind) + ']' : '') + (a.main ? ' <span class="ofr-tag">основной</span>' : '');
    }).join('<br>'), 'bin');
  }
  r('Events', evs(m.events || []), (m.events && m.events.length) ? 'bin' : '');
  return '<table>' + rows.join('') + '</table>';
}
function V_alias(R) { return (R && R.alias) || ''; }
function inspectBtn(b, R) {
  var lang = R.lang, rows = [], bar = R.m.C[b.ck], prov = b.auto ? 'auto' : 'bin';
  function r(a, v, p) { rows.push('<tr><td>' + a + '</td><td>' + v + (p ? '<span class="prov ' + p + '">' + PROV[p] + '</span>' : '') + '</td></tr>'); }
  r('Tag', '&lt;Button&gt; (кнопка командной панели)', prov);
  r('name', '<b>' + esc(b.name) + '</b>', prov);
  if (b.id) r('id', esc(b.id), 'bin');
  if (bar) r('CommandBar', '<span class="ofr-link" data-ofr-sel="' + bar.k + '">' + esc(bar.tag) + ' <b>' + esc(bar.name) + '</b></span>');
  if (b.parentBk >= 0 && R.m.B[b.parentBk]) {
    var pb = R.m.B[b.parentBk];
    r('Родительское подменю', '<span class="ofr-link" data-ofr-btn="' + pb.bk + '"><b>' + esc(pb.name) + '</b> (' + esc(T(pb.caps, lang) || pb.name) + ')</span>');
  }
  r('Kind', esc(b.kind), prov);
  var cap = T(b.caps, lang), tip = T(b.tips, lang);
  r('Caption', esc(cap), Object.keys(b.caps).length ? prov : '');
  if (tip) r('Tooltip', esc(tip), prov);
  r('Representation', esc(b.rep || 'Auto'), b.rep && b.rep !== 'Auto' ? 'bin' : 'def');
  if (b.checked) r('Checked', 'true', 'bin');
  if (b.actionKind === 'Handler' && b.action) {
    r('Действие (BSL)', '<span class="ev" data-h="' + esc(b.action) + '">' + esc(b.action) + '()</span>', 'bin');
  } else if (b.action) {
    var stdTitle = STD_NAMES[b.action] ? ' — ' + STD_NAMES[b.action] : '';
    r('Действие', esc(b.action + stdTitle), prov);
  } else if (b.actionKind === 'OpenMovements') {
    r('Действие', 'Открыть движения документа по регистрам (Дт/Кт)', 'bin');
  }
  if (b.kids && b.kids.length) {
    r('Пункты подменю (' + b.kids.length + ')', b.kids.map(function (k2) {
      var kc = T(k2.caps, lang) || k2.name;
      var kact = k2.actionKind === 'Handler' && k2.action ? ' → <span class="ev" data-h="' + esc(k2.action) + '">' + esc(k2.action) + '()</span>' : '';
      return '<span class="ofr-link" data-ofr-btn="' + k2.bk + '"><b>' + esc(k2.name) + '</b></span>' + (kc && kc !== k2.name ? ' («' + esc(kc) + '»)' : '') + (k2.kind !== 'Action' ? ' [' + esc(k2.kind) + ']' : '') + kact;
    }).join('<br>'), 'bin');
  }
  return '<table>' + rows.join('') + '</table><pre class="hljs">' + hl(b.xml, 'xml') + '</pre>';
}
function evs(list) {
  return list.map(function (e) { return '&lt;' + esc(e.name) + '&gt; <span class="ev" data-h="' + esc(e.handler) + '">' + esc(e.handler) + '</span>'; }).join('<br>');
}

var PROC_RE = /^\s*(Процедура|Функция|Procedure|Function)\s+([\p{L}\p{N}_]+)/iu;
var MAXLINES = 20000;
function hl(text, lang) {
  if (root.hljs && lang && root.hljs.getLanguage(lang)) {
    try { return root.hljs.highlight(text, { language: lang, ignoreIllegals: true }).value; } catch (e) {}
  }
  return esc(text);
}
function splitHlLines(html) {
  var open = [], tagRe = /<(\/?)span([^>]*)>/g;
  return html.split('\n').map(function (line) {
    var prefix = open.join(''), m;
    tagRe.lastIndex = 0;
    while ((m = tagRe.exec(line)) !== null) {
      if (m[1] === '/') open.pop();
      else open.push('<span' + m[2] + '>');
    }
    var suffix = new Array(open.length + 1).join('</span>');
    return prefix + line + suffix;
  });
}
function code(text, prefix, pm, lang) {
  var norm = (text || '').replace(/\r\n?/g, '\n'), lines = norm.split('\n'), rows = [];
  lines.forEach(function (line, i) {
    var m = PROC_RE.exec(line);
    if (m && pm && pm[m[2].toLowerCase()] === undefined) pm[m[2].toLowerCase()] = i + 1;
  });
  if (pm && !prefix) return '';
  var slice = lines.length > MAXLINES ? lines.slice(0, MAXLINES) : lines;
  var hlLines = splitHlLines(hl(slice.join('\n'), lang || '1c'));
  for (var i = 0; i < slice.length; i++) {
    var n = i + 1, isProc = PROC_RE.test(slice[i]);
    rows.push('<div class="l' + (isProc ? ' proc' : '') + '" id="' + prefix + n + '"><i>' + n + '</i><span>' + hlLines[i] + '</span></div>');
  }
  if (lines.length > MAXLINES) rows.push('<div class="l"><i></i><span>… показаны первые ' + MAXLINES + ' строк из ' + lines.length + '</span></div>');
  return rows.join('');
}

// ------------------------------------------------------------------ viewer
/* o: {host, tree, query, info, langSel, modeSel, hidSel, zoomSel, inspecting(), onClickCanvas(c), onProc(name), onSelect(c)} */
function create(o) {
  var R = { m: null, lang: 'ru', mode: 'designer', theme: 'v82', showHidden: false, zoom: '100', active: {}, visOverride: {}, treeFilter: 'all' }, sel = -1, selB = -1, selCol = -1, selRow = -1, selAttr = -1, selForm = false, hov = -1, hovB = -1, hovCol = -1, hovRow = -1, popEl = null, V = {};
  function $(sel2) { return o.host.querySelector(sel2); }
  function isChrome0() { return document.body && document.body.classList.contains('chrome-0'); }
  function closePop() { if (popEl && popEl.parentNode) popEl.parentNode.removeChild(popEl); popEl = null; }
  function btnBoxHtml(bk, cls) {
    if (bk < 0 || !R.m || !R.m.B || !R.m.B[bk]) return '';
    var b = R.m.B[bk], cv = $('#canvas'), el = o.host.querySelector('#b' + bk);
    if (!el && b.parentBk >= 0) el = o.host.querySelector('#b' + b.parentBk);
    if (!el || !cv) return '';
    var cr = cv.getBoundingClientRect(), er = el.getBoundingClientRect(), sc = cr.width > 0 && R.m.width > 0 ? cr.width / R.m.width : 1;
    if (er.width <= 0 && er.height <= 0) return '';
    var x = Math.round((er.left - cr.left) / sc), y = Math.round((er.top - cr.top) / sc), w = Math.round(er.width / sc), h = Math.round(er.height / sc);
    var badge = '';
    if (cls === 'sel') {
      var cap = T(b.caps, R.lang) || b.name;
      badge = '<span class="ofr-badge' + (y < 16 ? ' bot' : '') + '">Button: ' + esc(b.name + (cap && cap !== b.name ? ' («' + cap + '»)' : '') + (b.action ? ' → ' + b.action : '')) + '</span>';
    }
    return '<div class="ofr-box ' + cls + '" style="left:' + x + 'px;top:' + y + 'px;width:' + Math.max(2, w) + 'px;height:' + Math.max(2, h) + 'px">' + badge + '</div>';
  }
  function rowBoxHtml(ck, cls) {
    if (ck < 0 || !R.m || !R.m.C[ck]) return '';
    var tbl = R.m.C[ck], cv = $('#canvas'), el = o.host.querySelector('#row' + ck);
    if (!cv || !el) return '';
    var cr = cv.getBoundingClientRect(), er = el.getBoundingClientRect(), sc = cr.width > 0 && R.m.width > 0 ? cr.width / R.m.width : 1;
    if (er.width <= 0 && er.height <= 0) return '';
    var x = Math.round((er.left - cr.left) / sc), y = Math.round((er.top - cr.top) / sc), w = Math.round(er.width / sc), h = Math.round(er.height / sc);
    var badge = '';
    if (cls === 'sel') {
      badge = '<span class="ofr-badge' + (y < 16 ? ' bot' : '') + '">Row: ' + esc(tbl.name + ' (строка данных · ' + tbl.cols.length + ' кол.)') + '</span>';
    }
    return '<div class="ofr-box ' + cls + '" style="left:' + x + 'px;top:' + y + 'px;width:' + Math.max(2, w) + 'px;height:' + Math.max(2, h) + 'px">' + badge + '</div>';
  }
  function colBoxHtml(colk, cls) {
    if (colk < 0 || !R.m || !R.m.Cols || !R.m.Cols[colk]) return '';
    var col = R.m.Cols[colk], tbl = R.m.C[col.ck], cv = $('#canvas'), el = o.host.querySelector('#colh' + colk), tel = o.host.querySelector('#c' + col.ck);
    if (!cv) return '';
    var cr = cv.getBoundingClientRect(), sc = cr.width > 0 && R.m.width > 0 ? cr.width / R.m.width : 1;
    var x, y, w, h, ghost = false;
    if (el && tel) {
      var er = el.getBoundingClientRect(), tr = tel.getBoundingClientRect();
      x = Math.round((er.left - cr.left) / sc);
      y = Math.round((er.top - cr.top) / sc);
      w = Math.round(er.width / sc);
      h = Math.max(18, Math.round((tr.bottom - er.top) / sc) - 16);
    } else if (tbl) {
      x = tbl.ax; y = tbl.ay; w = Math.min(col.w, tbl.w); h = tbl.h; ghost = true;
    } else return '';
    var badge = '';
    if (cls === 'sel') {
      var hdr = T(col.headers, R.lang) || col.name;
      badge = '<span class="ofr-badge' + (y < 16 ? ' bot' : '') + '">Column: ' + esc((tbl ? tbl.name + '.' : '') + col.name + (hdr && hdr !== col.name ? ' («' + hdr + '»)' : '') + ' · ' + col.w + 'px' + (col.visible ? '' : ' · скрыта')) + '</span>';
    }
    return '<div class="ofr-box ' + cls + (ghost ? ' ghost' : '') + '" style="left:' + x + 'px;top:' + y + 'px;width:' + Math.max(2, w) + 'px;height:' + Math.max(2, h) + 'px">' + badge + '</div>';
  }
  function boxHtml(k, cls) {
    if (k < 0 || !R.m || !R.m.C[k]) return '';
    var c = R.m.C[k], cv = $('#canvas'), el = o.host.querySelector('#c' + k), x = c.ax, y = c.ay, w = c.w, h = c.h, ghost = !el;
    if (el && cv) {
      var cr = cv.getBoundingClientRect(), er = el.getBoundingClientRect(), sc = cr.width > 0 && R.m.width > 0 ? cr.width / R.m.width : 1;
      if (er.width > 0 || er.height > 0) { x = Math.round((er.left - cr.left) / sc); y = Math.round((er.top - cr.top) / sc); w = Math.round(er.width / sc); h = Math.round(er.height / sc); }
    }
    var badge = '';
    if (cls === 'sel') {
      var tags = [c.tag + ': ' + c.name];
      if (ghost) tags.push('скрыт');
      if (c.tabless) {
        var pgs = (R.m.kidsOf[c.k] || []).filter(function (p) { return p.tag === 'Page'; });
        var curP = R.m.C[R.active[c.k]];
        if (curP) tags.push('слой ' + (pgs.indexOf(curP) + 1) + '/' + pgs.length + ': ' + (T(curP.caps, R.lang) || curP.name));
      }
      if (R.m.ov[c.k] && R.m.ov[c.k].length) tags.push('⇄' + (R.m.ov[c.k].length + 1) + ' (Alt+клик)');
      badge = '<span class="ofr-badge' + (y < 16 ? ' bot' : '') + '">' + esc(tags.join(' · ')) + '</span>';
    }
    return '<div class="ofr-box ' + cls + (ghost ? ' ghost' : '') + '" style="left:' + x + 'px;top:' + y + 'px;width:' + Math.max(2, w) + 'px;height:' + Math.max(2, h) + 'px">' + badge + '</div>';
  }
  function updateOverlay() {
    if (isChrome0() || !R.m || R.m.error) return;
    var cv = $('#canvas'); if (!cv) return;
    var ov = cv.querySelector('.ofr-ov');
    if (sel < 0 && selB < 0 && selCol < 0 && selRow < 0 && hov < 0 && hovB < 0 && hovCol < 0 && hovRow < 0) { if (ov) ov.innerHTML = ''; return; }
    if (!ov) { ov = document.createElement('div'); ov.className = 'ofr-ov'; cv.appendChild(ov); }
    var hHtml = hovB >= 0 ? (hovB !== selB ? btnBoxHtml(hovB, 'hl') : '')
      : (hovCol >= 0 ? (hovCol !== selCol ? colBoxHtml(hovCol, 'hl') : '')
      : (hovRow >= 0 ? (hovRow !== selRow ? rowBoxHtml(hovRow, 'hl') : '')
      : (hov >= 0 && hov !== sel ? boxHtml(hov, 'hl') : '')));
    var sHtml = selB >= 0 ? btnBoxHtml(selB, 'sel')
      : (selCol >= 0 ? colBoxHtml(selCol, 'sel')
      : (selRow >= 0 ? rowBoxHtml(selRow, 'sel')
      : (sel >= 0 ? boxHtml(sel, 'sel') : '')));
    ov.innerHTML = hHtml + sHtml;
  }
  function clearHover() {
    o.host.querySelectorAll('.hl').forEach(function (x) { if (!x.classList.contains('ofr-box')) x.classList.remove('hl'); });
    if (o.tree) o.tree.querySelectorAll('.hl').forEach(function (x) { x.classList.remove('hl'); });
    hov = -1; hovB = -1; hovCol = -1; hovRow = -1;
  }
  function setHover(kind, idx) {
    if (kind === 'c' && hov === idx && hovB < 0 && hovCol < 0 && hovRow < 0) return;
    if (kind === 'b' && hovB === idx && hov < 0 && hovCol < 0 && hovRow < 0) return;
    if (kind === 'col' && hovCol === idx && hov < 0 && hovB < 0 && hovRow < 0) return;
    if (kind === 'row' && hovRow === idx && hov < 0 && hovB < 0 && hovCol < 0) return;
    if (!kind && hov < 0 && hovB < 0 && hovCol < 0 && hovRow < 0) return;
    clearHover();
    if (kind === 'c' && idx >= 0) {
      hov = idx;
      var e = o.host.querySelector('#c' + idx); if (e) e.classList.add('hl');
      var t = o.tree && o.tree.querySelector('[data-k="' + idx + '"]'); if (t) t.classList.add('hl');
    } else if (kind === 'b' && idx >= 0) {
      hovB = idx;
      var eb = o.host.querySelector('#b' + idx); if (eb) eb.classList.add('hl');
      var tb = o.tree && o.tree.querySelector('[data-b="' + idx + '"]'); if (tb) tb.classList.add('hl');
    } else if (kind === 'col' && idx >= 0) {
      hovCol = idx;
      var eh = o.host.querySelector('#colh' + idx); if (eh) eh.classList.add('hl');
      var ec = o.host.querySelector('#colc' + idx); if (ec) ec.classList.add('hl');
      var tc = o.tree && o.tree.querySelector('[data-col="' + idx + '"]'); if (tc) tc.classList.add('hl');
    } else if (kind === 'row' && idx >= 0) {
      hovRow = idx;
      var er = o.host.querySelector('#row' + idx); if (er) er.classList.add('hl');
      var tr = o.tree && o.tree.querySelector('[data-row="' + idx + '"]'); if (tr) tr.classList.add('hl');
    }
    updateOverlay();
  }
  function mark(k, cls, on) {
    if (cls === 'hl') { setHover(on ? 'c' : '', k); return; }
    var e = o.host.querySelector('#c' + k); if (e) e.classList.toggle(cls, on);
    var t = o.tree && o.tree.querySelector('[data-k="' + k + '"]'); if (t) t.classList.toggle(cls, on);
    updateOverlay();
  }
  function markBtn(bk, cls, on) {
    if (cls === 'hl') { setHover(on ? 'b' : '', bk); return; }
    var e = o.host.querySelector('#b' + bk); if (e) e.classList.toggle(cls, on);
    var t = o.tree && o.tree.querySelector('[data-b="' + bk + '"]'); if (t) t.classList.toggle(cls, on);
    updateOverlay();
  }
  function markCol(colk, cls, on) {
    if (cls === 'hl') { setHover(on ? 'col' : '', colk); return; }
    var eh = o.host.querySelector('#colh' + colk); if (eh) eh.classList.toggle(cls, on);
    var ec = o.host.querySelector('#colc' + colk); if (ec) ec.classList.toggle(cls, on);
    var t = o.tree && o.tree.querySelector('[data-col="' + colk + '"]'); if (t) t.classList.toggle(cls, on);
    updateOverlay();
  }
  function markRow(ck, cls, on) {
    if (cls === 'hl') { setHover(on ? 'row' : '', ck); return; }
    var er = o.host.querySelector('#row' + ck); if (er) er.classList.toggle(cls, on);
    var tr = o.tree && o.tree.querySelector('[data-row="' + ck + '"]'); if (tr) tr.classList.toggle(cls, on);
    updateOverlay();
  }
  function clearSubSel() {
    o.host.querySelectorAll('.sel').forEach(function (x) { if (!x.classList.contains('ofr-box')) x.classList.remove('sel'); });
    if (o.tree) o.tree.querySelectorAll('.sel').forEach(function (x) { x.classList.remove('sel'); });
    sel = -1; selB = -1; selCol = -1; selRow = -1; selAttr = -1; selForm = false;
  }
  function renderBar() {
    if (isChrome0() || !R.m || R.m.error) return '';
    var rows = [];
    (R.m.tablessPanels || []).forEach(function (pan) {
      var pgs = (R.m.kidsOf[pan.k] || []).filter(function (x) { return x.tag === 'Page'; });
      if (pgs.length < 2) return;
      var cur = R.active[pan.k];
      rows.push('<div class="ofr-brow"><span class="ofr-blbl" data-ofr-sel="' + pan.k + '" title="Панель без корешков закладок (ShowTabs=DontUse)">' + ICON.layers + ' Слои <b>' + esc(pan.name) + '</b>:</span>' +
        pgs.map(function (pg, i) {
          var cap2 = T(pg.caps, R.lang) || pg.name, on = pg.k === cur;
          return '<span class="ofr-chip' + (on ? ' on' : '') + (pg.visible ? '' : ' hid') + '" data-ofr-page="' + pg.k + '">' + (i + 1) + '. ' + esc(cap2) + '</span>';
        }).join('') + '</div>');
    });
    var hidK = [], shwK = [];
    Object.keys(R.visOverride).forEach(function (ks) {
      var k = +ks, c = R.m.C[k]; if (!c) return;
      if (R.visOverride[k]) shwK.push(c); else hidK.push(c);
    });
    if (hidK.length || shwK.length) {
      var parts = [];
      if (hidK.length) parts.push('<span class="ofr-blbl">Скрыто вручную:</span>' + hidK.map(function (c) {
        return '<span class="ofr-chip off" data-ofr-unoverride="' + c.k + '" title="Вернуть видимость">× ' + esc(c.name || c.tag) + '</span>';
      }).join(''));
      if (shwK.length) parts.push('<span class="ofr-blbl">Показано скрытых:</span>' + shwK.map(function (c) {
        return '<span class="ofr-chip shw" data-ofr-unoverride="' + c.k + '" title="Сбросить показ">× ' + esc(c.name || c.tag) + '</span>';
      }).join(''));
      parts.push('<span class="ofr-chip rst" data-ofr-reset="1">Сбросить всё</span>');
      rows.push('<div class="ofr-brow">' + parts.join(' ') + '</div>');
    }
    return rows.length ? '<div class="ofr-bar">' + rows.join('') + '</div>' : '';
  }
  function refreshBar() {
    var old = $('.ofr-bar'), html = renderBar();
    if (old) {
      if (html) old.outerHTML = html; else old.parentNode.removeChild(old);
    } else if (html) {
      o.host.insertAdjacentHTML('afterbegin', html);
    }
  }
  function activatePage(pk) {
    var p = R.m.C[pk].parent; R.active[p] = pk;
    R.m.C.forEach(function (s) {
      if (s.parent !== p || s.tag !== 'Page') return;
      var on = s.k === pk, e = o.host.querySelector('#c' + s.k); if (e) e.classList.toggle('on', on);
      o.host.querySelectorAll('.ptab[data-k="' + s.k + '"]').forEach(function (t) { t.classList.toggle('on', on); });
    });
    refreshBar();
    updateOverlay();
    if (sel >= 0 && o.info) o.info.innerHTML = inspect(R.m.C[sel], R);
  }
  function reveal(k) {
    var chain = [], cur = R.m.C[k];
    while (cur) { if (cur.tag === 'Page') chain.unshift(cur.k); cur = cur.parent >= 0 ? R.m.C[cur.parent] : null; }
    chain.forEach(activatePage);
  }
  V.setVis = function (k, val) {
    if (!R.m || !R.m.C[k]) return;
    if (val === null || val === undefined) {
      delete R.visOverride[k];
    } else {
      R.visOverride[k] = !!val;
      if (val) {
        for (var p = R.m.C[k].parent; p >= 0; p = R.m.C[p].parent) {
          var anc = R.m.C[p];
          if (!vis(anc, R)) R.visOverride[anc.k] = true;
        }
        reveal(k);
      }
    }
    draw(); buildTree();
    if (sel >= 0 && o.info) o.info.innerHTML = inspect(R.m.C[sel], R);
  };
  V.toggleVis = function (k) {
    if (!R.m || !R.m.C[k]) return;
    var c = R.m.C[k], cur = vis(c, R);
    V.setVis(k, !cur);
  };
  V.resetVis = function () {
    R.visOverride = {};
    draw(); buildTree();
    if (sel >= 0 && o.info) o.info.innerHTML = inspect(R.m.C[sel], R);
  };
  function matchFilter(c, q) {
    if (q && (c.tag + ' ' + c.name + ' ' + T(c.caps, R.lang)).toLowerCase().indexOf(q) < 0) return false;
    if (R.treeFilter === 'hidden') return !c.visible || (c.k in R.visOverride);
    if (R.treeFilter === 'overlaps') return !!(R.m.ov[c.k] && R.m.ov[c.k].length);
    if (R.treeFilter === 'layers') return c.tag === 'Panel' || c.tag === 'Page' || c.tag === 'UsualGroup';
    return true;
  }
  function matchBtn(b, q) {
    if (!q) return R.treeFilter === 'all';
    return ('button ' + b.name + ' ' + T(b.caps, R.lang) + ' ' + b.action).toLowerCase().indexOf(q) >= 0;
  }
  function renderTreeBtns(list, baseDepth, q, keepB) {
    var out = '';
    (list || []).forEach(function (b) {
      if (b.kind === 'Separator') return;
      if ((q || R.treeFilter !== 'all') && !keepB[b.bk]) return;
      var cap2 = T(b.caps, R.lang), subN = (b.kids || []).length;
      var badges = '';
      if (b.kind === 'Submenu') badges += ' <span class="ofr-b lyr">меню' + (subN ? ' ' + subN : '') + '</span>';
      if (b.actionKind === 'Handler' && b.action) badges += ' <span class="ofr-b act">ƒ</span>';
      out += '<div data-b="' + b.bk + '" class="ofr-brow-tree" style="padding-left:' + (8 + baseDepth * 12) + 'px">' +
        '<span class="ofr-t">Button</span> <b>' + esc(b.name) + '</b> ' + esc(cap2 && cap2 !== b.name ? cap2 : '') + badges + '</div>';
      if (subN) out += renderTreeBtns(b.kids, baseDepth + 1, q, keepB);
    });
    return out;
  }
  function matchCol(col, q) {
    if (q && ('column ' + col.name + ' ' + T(col.headers, R.lang) + ' ' + col.dpName).toLowerCase().indexOf(q) < 0) return false;
    if (R.treeFilter === 'hidden') return !col.visible;
    return R.treeFilter === 'all';
  }
  function renderTreeCols(list, baseDepth, q, keepCol) {
    var out = '';
    (list || []).forEach(function (col) {
      if ((q || R.treeFilter !== 'all') && !keepCol[col.colk]) return;
      var hdr = T(col.headers, R.lang), badges = '';
      if (!col.visible) badges += ' <span class="ofr-b hid">скрыта</span>';
      if (col.events && col.events.length) badges += ' <span class="ofr-b act">ƒ</span>';
      out += '<div data-col="' + col.colk + '" class="ofr-brow-tree' + (col.visible ? '' : ' ofr-row-off') + '" style="padding-left:' + (8 + baseDepth * 12) + 'px">' +
        '<span class="ofr-t">Column</span> <b>' + esc(col.name) + '</b> ' + esc(hdr && hdr !== col.name ? hdr : '') + ' <span class="ofr-tag">' + col.w + 'px</span>' + badges + '</div>';
    });
    return out;
  }
  function buildTree() {
    if (!o.tree || !R.m || R.m.error) return;
    var q = ((o.query && o.query.value) || '').toLowerCase(), h = '', keep = {}, direct = {}, keepB = {}, keepCol = {};
    if (q || R.treeFilter !== 'all') {
      R.m.C.forEach(function (c) {
        if (matchFilter(c, q)) {
          direct[c.k] = 1; keep[c.k] = 1;
          for (var p = c.parent; p >= 0; p = R.m.C[p].parent) keep[p] = 1;
        }
      });
      (R.m.B || []).forEach(function (b) {
        if (!b.auto && matchBtn(b, q)) {
          keepB[b.bk] = 1;
          for (var pb = b.parentBk; pb >= 0; pb = R.m.B[pb].parentBk) keepB[pb] = 1;
          keep[b.ck] = 1;
          for (var p2 = R.m.C[b.ck].parent; p2 >= 0; p2 = R.m.C[p2].parent) keep[p2] = 1;
        }
      });
      (R.m.Cols || []).forEach(function (col) {
        if (matchCol(col, q)) {
          keepCol[col.colk] = 1;
          keep[col.ck] = 1;
          for (var p3 = R.m.C[col.ck].parent; p3 >= 0; p3 = R.m.C[p3].parent) keep[p3] = 1;
        }
      });
    }
    if (R.treeFilter === 'all') {
      var fCap = T(R.m.caps, R.lang) || R.m.formName || V.alias || 'Form';
      if (!q || ('form ' + R.m.formName + ' ' + fCap).toLowerCase().indexOf(q) >= 0) {
        var fBadge = (R.m.events && R.m.events.length) ? ' <span class="ofr-b act">ƒ' + R.m.events.length + '</span>' : '';
        h += '<div data-form="1" class="ofr-brow-tree' + (selForm ? ' sel' : '') + '" style="padding-left:4px"><span class="ofr-t">Form</span> <b>' + esc(R.m.formName || V.alias || 'Form') + '</b> ' + esc(fCap) + fBadge + '</div>';
      }
      (R.m.A || []).forEach(function (a) {
        if (q && ('attribute ' + a.name + ' ' + (a.vt ? a.vt.kind : '')).toLowerCase().indexOf(q) < 0) return;
        var aBadge = (a.main ? ' <span class="ofr-b lyr">основной</span>' : '') + (a.vt ? ' <span class="ofr-tag">' + esc(a.vt.kind) + '</span>' : '');
        h += '<div data-a="' + a.ak + '" class="ofr-brow-tree' + (selAttr === a.ak ? ' sel' : '') + '" style="padding-left:16px"><span class="ofr-t">Attribute</span> <b>' + esc(a.name) + '</b>' + aBadge + '</div>';
      });
    }
    R.m.C.forEach(function (c) {
      if ((q || R.treeFilter !== 'all') && !keep[c.k]) return;
      var d = c.depth || 0, eff = vis(c, R), hasOv = c.k in R.visOverride, ovN = (R.m.ov[c.k] || []).length;
      var dim = (q || R.treeFilter !== 'all') && !direct[c.k] ? ' ofr-ctx' : '';
      var badges = '';
      if (!c.visible) badges += ' <span class="ofr-b hid">скрыт</span>';
      if (hasOv) badges += ' <span class="ofr-b ovr">' + (eff ? 'показан' : 'скрыт вручную') + '</span>';
      if (c.tabless) badges += ' <span class="ofr-b lyr">слои</span>';
      if (ovN) badges += ' <span class="ofr-b ovl" title="Перекрывается с ' + ovN + ' элем.">⇄' + ovN + '</span>';
      var eye = '<span class="ofr-eye' + (eff ? '' : ' off') + (hasOv ? ' mod' : '') + '" data-ofr-eye="' + c.k + '" title="' + (eff ? 'Скрыть элемент (H)' : 'Показать элемент (H)') + '">' + (eff ? ICON.eye : ICON.eyeOff) + '</span>';
      h += '<div data-k="' + c.k + '" class="' + (eff ? '' : 'ofr-row-off') + dim + '" style="padding-left:' + (4 + d * 12) + 'px">' +
        eye + '<span class="ofr-t">' + esc(c.tag) + '</span> <b>' + esc(c.name) + '</b> ' + esc(T(c.caps, R.lang)) + badges + '</div>';
      if (c.tag === 'CommandBar' && R.m.btnsOf && R.m.btnsOf[c.k]) {
        h += renderTreeBtns(R.m.btnsOf[c.k], d + 1, q, keepB);
      }
      if (c.tag === 'Table' && c.cols && c.cols.length) {
        if (R.treeFilter === 'all' && (!q || ('row строка ' + c.name).toLowerCase().indexOf(q) >= 0)) {
          var rBadge = (c.events && c.events.length) ? ' <span class="ofr-b act">ƒ' + c.events.length + '</span>' : '';
          h += '<div data-row="' + c.k + '" class="ofr-brow-tree" style="padding-left:' + (8 + (d + 1) * 12) + 'px"><span class="ofr-t">Row</span> <b>Строка данных</b> <span class="ofr-tag">' + c.cols.length + ' кол.</span>' + rBadge + '</div>';
        }
        h += renderTreeCols(c.cols, d + 1, q, keepCol);
      }
    });
    o.tree.innerHTML = h || '<div class="err">Нет подходящих элементов</div>';
    if (sel >= 0) mark(sel, 'sel', true);
    if (selB >= 0) markBtn(selB, 'sel', true);
    if (selCol >= 0) markCol(selCol, 'sel', true);
    if (selRow >= 0) markRow(selRow, 'sel', true);
  }
  function draw() {
    var m = R.m, h;
    closePop();
    if (!m || m.error) { o.host.innerHTML = '<div class="err">' + esc(m ? m.error : 'Form.xml недоступен для этой формы') + '</div>'; return; }
    // Ensure initial active pages are populated before renderBar reads R.active
    (m.tablessPanels || []).forEach(function (pan) {
      if (R.active[pan.k] === undefined) {
        var pgs = (m.kidsOf[pan.k] || []).filter(function (x) { return x.tag === 'Page'; });
        if (pgs.length) R.active[pan.k] = (pgs.filter(function (p) { return p.visible; })[0] || pgs[0]).k;
      }
    });
    var title = T(m.caps, R.lang) || m.formName || V.alias || '';
    var topKids = ordered(m.kidsOf[-1]).filter(function (c) { return vis(c, R); });
    var rootPages = topKids.filter(function (x) { return x.tag === 'Page'; });
    var bodyHtml = '';
    if (rootPages.length) {
      var seq = rootPages.slice().sort(function (a, b) { return a.k - b.k; });
      var first = R.active[-1];
      if (!seq.some(function (p) { return p.k === first; })) { first = (seq.filter(function (p) { return p.visible; })[0] || seq[0]).k; R.active[-1] = first; }
      if (seq.length > 1 || (m.showTabs && m.showTabs !== '0')) bodyHtml += tabsHtml({ k: -1, e: null }, seq, first, R);
      bodyHtml += pagesHtml({ k: -1, e: null }, seq, first, R);
      topKids = topKids.filter(function (x) { return x.tag !== 'Page'; });
    }
    bodyHtml += topKids.map(function (c) { return ctl(c, R); }).join('');
    h = renderBar() + '<div class="zoomer"><div class="formwrap" style="width:' + (m.width + 2) + 'px"><div class="form-title">' + (m.capPic ? '<span class="pic16">' + ICON.picture + '</span>' : '') + esc(title) + '</div>' +
      '<div id="canvas" class="theme-' + esc(R.theme) + ' mode-' + esc(R.mode) + '" style="width:' + m.width + 'px;height:' + m.height + 'px">' +
      bodyHtml + '</div></div></div>';
    o.host.innerHTML = h;
    if (sel >= 0) mark(sel, 'sel', true);
    V.applyZoom();
    updateOverlay();
  }
  V.applyZoom = function () {
    var fw = $('.formwrap'), z = $('.zoomer'); if (!fw || !z) return;
    var w = fw.offsetWidth, hgt = fw.offsetHeight, s = 1;
    if (R.zoom === 'fit') { var avail = o.host.clientWidth; s = avail > 0 && w > 0 ? avail / w : 1; } else s = (parseInt(R.zoom, 10) || 100) / 100;
    fw.style.transform = s === 1 ? '' : 'scale(' + s + ')';
    z.style.width = Math.ceil(w * s) + 'px'; z.style.height = Math.ceil(hgt * s) + 'px';
    updateOverlay();
  };
  V.select = function (k) {
    reveal(k);
    clearSubSel();
    sel = k; mark(k, 'sel', true);
    var c = R.m.C[k];
    if (o.info) { o.info.innerHTML = inspect(c, R); }
    if (o.onSelect) o.onSelect(c);
    var t = o.tree && o.tree.querySelector('[data-k="' + k + '"]'); if (t && t.scrollIntoView) t.scrollIntoView({ block: 'nearest' });
  };
  V.selectButton = function (bk) {
    if (!R.m || !R.m.B || !R.m.B[bk]) return;
    var b = R.m.B[bk];
    reveal(b.ck);
    clearSubSel();
    selB = bk; markBtn(bk, 'sel', true);
    if (o.info) o.info.innerHTML = inspectBtn(b, R);
    var t = o.tree && o.tree.querySelector('[data-b="' + bk + '"]'); if (t && t.scrollIntoView) t.scrollIntoView({ block: 'nearest' });
  };
  V.selectCol = function (colk) {
    if (!R.m || !R.m.Cols || !R.m.Cols[colk]) return;
    var col = R.m.Cols[colk];
    reveal(col.ck);
    clearSubSel();
    selCol = colk; markCol(colk, 'sel', true);
    if (o.info) o.info.innerHTML = inspectCol(col, R);
    var t = o.tree && o.tree.querySelector('[data-col="' + colk + '"]'); if (t && t.scrollIntoView) t.scrollIntoView({ block: 'nearest' });
  };
  V.selectRow = function (ck) {
    if (!R.m || !R.m.C[ck]) return;
    var c = R.m.C[ck];
    reveal(ck);
    clearSubSel();
    selRow = ck; markRow(ck, 'sel', true);
    if (o.info) o.info.innerHTML = inspectRow(c, R);
    var t = o.tree && o.tree.querySelector('[data-row="' + ck + '"]'); if (t && t.scrollIntoView) t.scrollIntoView({ block: 'nearest' });
  };
  V.selectAttr = function (ak) {
    if (!R.m || !R.m.A || !R.m.A[ak]) return;
    clearSubSel();
    selAttr = ak;
    var t = o.tree && o.tree.querySelector('[data-a="' + ak + '"]'); if (t) { t.classList.add('sel'); if (t.scrollIntoView) t.scrollIntoView({ block: 'nearest' }); }
    if (o.info) o.info.innerHTML = inspectAttr(R.m.A[ak], R);
    updateOverlay();
  };
  V.selectForm = function () {
    if (!R.m || R.m.error) return;
    clearSubSel();
    selForm = true;
    var t = o.tree && o.tree.querySelector('[data-form]'); if (t) { t.classList.add('sel'); if (t.scrollIntoView) t.scrollIntoView({ block: 'nearest' }); }
    if (o.info) o.info.innerHTML = inspectForm(R);
    updateOverlay();
  };
  function showSubmenuPopup(anchorEl, b) {
    closePop();
    if (!b.kids || !b.kids.length) return;
    popEl = document.createElement('div');
    popEl.className = 'ofr-pop';
    var cap = T(b.caps, R.lang) || b.name;
    popEl.innerHTML = '<div class="ofr-ph">Меню «' + esc(cap) + '» (' + b.kids.length + ')</div>' + b.kids.map(function (k2) {
      if (k2.kind === 'Separator') return '<div class="ofr-psep"></div>';
      var kc = T(k2.caps, R.lang) || k2.name, subN = (k2.kids || []).length;
      return '<div class="ofr-pi' + (k2.bk === selB ? ' on' : '') + '" data-ofr-btn="' + k2.bk + '">' +
        '<span class="ofr-pt">' + esc(kc) + (kc !== k2.name ? ' <span class="ofr-tag">(' + esc(k2.name) + ')</span>' : '') +
        (subN ? ' ▸' : '') + (k2.actionKind === 'Handler' && k2.action ? ' · <b>' + esc(k2.action) + '()</b>' : '') + '</span></div>';
    }).join('');
    document.body.appendChild(popEl);
    var r = anchorEl.getBoundingClientRect(), pw = popEl.offsetWidth || 220, ph = popEl.offsetHeight || 120;
    popEl.style.left = Math.min(window.innerWidth - pw - 8, Math.max(4, r.left)) + 'px';
    popEl.style.top = Math.min(window.innerHeight - ph - 8, Math.max(4, r.bottom + 2)) + 'px';
  }
  function stackAt(clientX, clientY) {
    var cv = $('#canvas'); if (!cv || !R.m || R.m.error) return [];
    var r = cv.getBoundingClientRect(), sc = r.width > 0 && R.m.width > 0 ? r.width / R.m.width : 1;
    var x = (clientX - r.left) / sc, y = (clientY - r.top) / sc;
    return R.m.C.filter(function (c) {
      if (c.tag === 'Page' || c.w <= 0 || c.h <= 0) return false;
      if (!isPageChainActive(c, R)) return false;
      return x >= c.ax && x <= c.ax + c.w && y >= c.ay && y <= c.ay + c.h;
    }).sort(function (a, b) {
      var av = vis(a, R) ? 1 : 0, bv = vis(b, R) ? 1 : 0;
      if (av !== bv) return bv - av;
      return b.ps - a.ps;
    });
  }
  function showStackPopup(clientX, clientY, list) {
    closePop();
    popEl = document.createElement('div');
    popEl.className = 'ofr-pop';
    popEl.innerHTML = '<div class="ofr-ph">Элементы в точке (' + list.length + ')</div>' + list.map(function (c) {
      var eff = vis(c, R), isSel = c.k === sel;
      return '<div class="ofr-pi' + (isSel ? ' on' : '') + (eff ? '' : ' off') + '" data-ofr-sel="' + c.k + '">' +
        '<span class="ofr-eye' + (eff ? '' : ' off') + '" data-ofr-eye="' + c.k + '" title="Скрыть/показать">' + (eff ? ICON.eye : ICON.eyeOff) + '</span>' +
        '<span class="ofr-pt">' + esc(c.tag) + ' <b>' + esc(c.name) + '</b>' + (c.visible ? '' : ' <i>[скрыт]</i>') + '</span></div>';
    }).join('');
    document.body.appendChild(popEl);
    var pw = popEl.offsetWidth || 220, ph = popEl.offsetHeight || 120;
    popEl.style.left = Math.min(window.innerWidth - pw - 8, Math.max(4, clientX)) + 'px';
    popEl.style.top = Math.min(window.innerHeight - ph - 8, Math.max(4, clientY)) + 'px';
  }
  function resolvePage(p) {
    if (p === null || p === undefined || p === '') return;
    var m = R.m, hit = null;
    m.C.forEach(function (c) { if (!hit && c.tag === 'Page' && (c.name === p || T(c.caps, 'ru') === p)) hit = c; });
    if (!hit && /^\d+$/.test(p)) {
      var idx = parseInt(p, 10);
      m.C.forEach(function (c) { if (!hit && c.tag === 'Page' && c.pi === idx) hit = c; });
    }
    if (hit) R.active[hit.parent] = hit.k;
  }
  V.load = function (xml, opts) {
    opts = opts || {};
    V.alias = opts.alias || '';
    R.m = xml ? parse(xml) : { error: 'Form.xml недоступен для этой формы' };
    R.alias = V.alias;
    R.active = {}; R.visOverride = {}; sel = -1; selB = -1; selCol = -1; selRow = -1; selAttr = -1; selForm = false; hov = -1; hovB = -1; hovCol = -1; hovRow = -1; closePop();
    if (!R.m.error) {
      V.langs = R.m.langs.slice();
      if (V.langs.indexOf(R.lang) < 0) R.lang = V.langs[0];
      if (o.langSel) o.langSel.innerHTML = V.langs.map(function (l) { return '<option value="' + esc(l) + '"' + (l === R.lang ? ' selected' : '') + '>' + esc(l) + '</option>'; }).join('');
      resolvePage(opts.page);
    }
    draw(); buildTree();
    if (o.info) o.info.innerHTML = '<div class="err">Выберите элемент формы (Alt+клик — выбор сквозь слои, ПКМ — список слоёв в точке, H — скрыть/показать)</div>';
    return R.m;
  };
  V.set = function (key, val) {
    R[key] = val;
    if (!R.m || R.m.error) return;
    if (key === 'zoom') { V.applyZoom(); return; }
    draw();
    if (key === 'lang' || key === 'treeFilter' || key === 'showHidden') buildTree();
    if (sel >= 0 && o.info) V.select(sel);
    else if (selB >= 0 && o.info) V.selectButton(selB);
    else if (selCol >= 0 && o.info) V.selectCol(selCol);
    else if (selRow >= 0 && o.info) V.selectRow(selRow);
    else if (selAttr >= 0 && o.info) V.selectAttr(selAttr);
    else if (selForm && o.info) V.selectForm();
  };
  Object.defineProperty(V, 'state', { get: function () { return R; } });
  if (o.langSel) o.langSel.onchange = function () { V.set('lang', o.langSel.value); };
  if (o.modeSel) o.modeSel.onchange = function () { V.set('mode', o.modeSel.value); };
  if (o.hidSel) o.hidSel.onchange = function () { V.set('showHidden', o.hidSel.checked); };
  if (o.zoomSel) o.zoomSel.onchange = function () { V.set('zoom', o.zoomSel.value); };
  if (o.query) {
    o.query.oninput = buildTree;
    if (!o.query.parentNode.querySelector('.ofr-fbar')) {
      o.query.insertAdjacentHTML('afterend', '<div class="ofr-fbar">' +
        [['all', 'Все'], ['layers', 'Слои/Панели'], ['overlaps', 'Перекрытия'], ['hidden', 'Скрытые']].map(function (p) {
          return '<span class="ofr-fpill' + (p[0] === 'all' ? ' on' : '') + '" data-ofr-flt="' + p[0] + '">' + p[1] + '</span>';
        }).join('') + '</div>');
    }
  }
  window.addEventListener('resize', function () { if (R.zoom === 'fit') V.applyZoom(); });
  function target(ev) { return ev.target.closest('#canvas .c'); }
  function inspecting() { return o.inspecting ? o.inspecting() : false; }
  document.addEventListener('mouseover', function (ev) {
    if (!R.m || R.m.error || !inspecting()) return;
    var bEl = ev.target.closest('#canvas [data-b]') || (o.tree && o.tree.contains(ev.target) ? ev.target.closest('[data-b]') : null) || ev.target.closest('[data-ofr-btn]');
    if (bEl) {
      var bk = +(bEl.dataset.b !== undefined ? bEl.dataset.b : bEl.dataset.ofrBtn);
      if (bk >= 0) { setHover('b', bk); return; }
    }
    var colEl = ev.target.closest('#canvas [data-col]') || (o.tree && o.tree.contains(ev.target) ? ev.target.closest('[data-col]') : null) || ev.target.closest('[data-ofr-col]');
    if (colEl) {
      var colk = +(colEl.dataset.col !== undefined ? colEl.dataset.col : colEl.dataset.ofrCol);
      if (colk >= 0) { setHover('col', colk); return; }
    }
    var rowEl = ev.target.closest('#canvas [data-row]') || (o.tree && o.tree.contains(ev.target) ? ev.target.closest('[data-row]') : null) || ev.target.closest('[data-ofr-row]');
    if (rowEl) {
      var rck = +(rowEl.dataset.row !== undefined ? rowEl.dataset.row : rowEl.dataset.ofrRow);
      if (rck >= 0) { setHover('row', rck); return; }
    }
    var ptEl = ev.target.closest('#canvas .ptab');
    if (ptEl && ptEl.dataset.k !== undefined) { setHover('c', +ptEl.dataset.k); return; }
    var cEl = target(ev);
    if (cEl && cEl.classList.contains('t-Page')) cEl = null;
    var e = cEl || (o.tree && o.tree.contains(ev.target) ? ev.target.closest('[data-k]') : null) || ev.target.closest('[data-ofr-sel]');
    var k = e ? +(e.dataset.k !== undefined ? e.dataset.k : e.dataset.ofrSel) : -1;
    if (k >= 0) setHover('c', k);
    else setHover('', -1);
  });
  document.addEventListener('mouseout', function (ev) {
    if (!R.m || R.m.error) return;
    if (!ev.relatedTarget || (!o.host.contains(ev.relatedTarget) && !(o.tree && o.tree.contains(ev.relatedTarget)) && !(o.info && o.info.contains(ev.relatedTarget)))) {
      setHover('', -1);
    }
  });
  document.addEventListener('contextmenu', function (ev) {
    if (!R.m || R.m.error || ev.shiftKey || (!inspecting() && !o.alwaysSelect)) return;
    var cv = $('#canvas');
    if (!cv || !cv.contains(ev.target)) return;
    var list = stackAt(ev.clientX, ev.clientY);
    if (list.length) { ev.preventDefault(); showStackPopup(ev.clientX, ev.clientY, list); }
  });
  document.addEventListener('keydown', function (ev) {
    if (!R.m || R.m.error || ev.ctrlKey || ev.metaKey) return;
    var tg = ev.target && ev.target.tagName;
    if (tg === 'INPUT' || tg === 'SELECT' || tg === 'TEXTAREA') return;
    if (!o.host.offsetParent && !isChrome0()) return;
    if (ev.key === 'Escape') { closePop(); return; }
    if ((ev.key === 'h' || ev.key === 'H' || ev.key === 'р' || ev.key === 'Р') && sel >= 0) {
      ev.preventDefault(); V.toggleVis(sel);
    }
  });
  document.addEventListener('click', function (ev) {
    if (!R.m || R.m.error) return;
    var eyeEl = ev.target.closest('[data-ofr-eye]');
    if (eyeEl) { V.toggleVis(+eyeEl.dataset.ofrEye); ev.stopPropagation(); return; }
    var unov = ev.target.closest('[data-ofr-unoverride]');
    if (unov) { V.setVis(+unov.dataset.ofrUnoverride, null); ev.stopPropagation(); return; }
    var rst = ev.target.closest('[data-ofr-reset]');
    if (rst) { V.resetVis(); ev.stopPropagation(); return; }
    var pgEl = ev.target.closest('[data-ofr-page]');
    if (pgEl && o.host.contains(pgEl) || (o.info && o.info.contains(pgEl))) {
      var pk = +pgEl.dataset.ofrPage, pg = R.m.C[pk];
      if (pg && !vis(pg, R)) V.setVis(pk, true);
      activatePage(pk);
      if (inspecting()) V.select(pk);
      ev.stopPropagation(); return;
    }
    var selEl = ev.target.closest('[data-ofr-sel]');
    if (selEl) {
      var sk = +selEl.dataset.ofrSel;
      closePop(); V.select(sk);
      if (o.onClickCanvas) o.onClickCanvas(R.m.C[sk]);
      ev.stopPropagation(); return;
    }
    var btnLink = ev.target.closest('[data-ofr-btn]');
    if (btnLink) {
      var bk2 = +btnLink.dataset.ofrBtn, bObj = R.m.B && R.m.B[bk2];
      closePop(); V.selectButton(bk2);
      if (bObj && bObj.kids && bObj.kids.length) {
        var anchor = o.host.querySelector('#b' + bk2) || btnLink;
        showSubmenuPopup(anchor, bObj);
      }
      if (o.onClickCanvas && bObj) o.onClickCanvas({ name: bObj.name, tag: 'Button', events: bObj.actionKind === 'Handler' && bObj.action ? [{ name: 'Action', handler: bObj.action }] : [] });
      ev.stopPropagation(); return;
    }
    var colLink = ev.target.closest('[data-ofr-col]');
    if (colLink) {
      var colk2 = +colLink.dataset.ofrCol, colObj = R.m.Cols && R.m.Cols[colk2];
      closePop(); V.selectCol(colk2);
      if (o.onClickCanvas && colObj) o.onClickCanvas({ name: (R.m.C[colObj.ck] ? R.m.C[colObj.ck].name + '.' : '') + colObj.name, tag: 'Column', events: colObj.events || [] });
      ev.stopPropagation(); return;
    }
    var rowLink = ev.target.closest('[data-ofr-row]');
    if (rowLink) {
      var rck2 = +rowLink.dataset.ofrRow, tbl2 = R.m.C && R.m.C[rck2];
      closePop(); V.selectRow(rck2);
      if (o.onClickCanvas && tbl2) o.onClickCanvas({ name: tbl2.name + ' (строка)', tag: 'Row', events: tbl2.events || [] });
      ev.stopPropagation(); return;
    }
    var attrLink = ev.target.closest('[data-ofr-attr]');
    if (attrLink) {
      closePop(); V.selectAttr(+attrLink.dataset.ofrAttr);
      ev.stopPropagation(); return;
    }
    var fltEl = ev.target.closest('[data-ofr-flt]');
    if (fltEl && o.tree && o.tree.parentNode.contains(fltEl)) {
      R.treeFilter = fltEl.dataset.ofrFlt;
      fltEl.parentNode.querySelectorAll('.ofr-fpill').forEach(function (x) { x.classList.toggle('on', x === fltEl); });
      buildTree(); ev.stopPropagation(); return;
    }
    closePop();
    var h = ev.target.closest('.ev'); if (h && h.dataset.h) { if (o.onProc) o.onProc(h.dataset.h); return; }
    var pt = ev.target.closest('.ptab');
    if (pt && o.host.contains(pt)) { activatePage(+pt.dataset.k); if (inspecting() || o.alwaysSelect) V.select(+pt.dataset.k); ev.stopPropagation(); return; }
    var cbBtn = ev.target.closest('#canvas [data-b]');
    if (cbBtn && o.host.contains(cbBtn) && !ev.altKey) {
      if (!inspecting() && !o.alwaysSelect) return;
      var bk3 = +cbBtn.dataset.b, b3 = R.m.B && R.m.B[bk3];
      if (b3) {
        V.selectButton(bk3);
        if (b3.kids && b3.kids.length) showSubmenuPopup(cbBtn, b3);
        if (o.onClickCanvas) o.onClickCanvas({ name: b3.name, tag: 'Button', events: b3.actionKind === 'Handler' && b3.action ? [{ name: 'Action', handler: b3.action }] : [] });
        ev.stopPropagation(); return;
      }
    }
    var tblCol = ev.target.closest('#canvas [data-col]');
    if (tblCol && o.host.contains(tblCol) && !ev.altKey) {
      if (!inspecting() && !o.alwaysSelect) return;
      var colk3 = +tblCol.dataset.col, col3 = R.m.Cols && R.m.Cols[colk3];
      if (col3) {
        V.selectCol(colk3);
        if (o.onClickCanvas) o.onClickCanvas({ name: (R.m.C[col3.ck] ? R.m.C[col3.ck].name + '.' : '') + col3.name, tag: 'Column', events: col3.events || [] });
        ev.stopPropagation(); return;
      }
    }
    var tblRow = ev.target.closest('#canvas [data-row]');
    if (tblRow && o.host.contains(tblRow) && !ev.altKey) {
      if (!inspecting() && !o.alwaysSelect) return;
      var rck3 = +tblRow.dataset.row, tbl3 = R.m.C && R.m.C[rck3];
      if (tbl3) {
        V.selectRow(rck3);
        if (o.onClickCanvas) o.onClickCanvas({ name: tbl3.name + ' (строка)', tag: 'Row', events: tbl3.events || [] });
        ev.stopPropagation(); return;
      }
    }
    var e = target(ev);
    if (e && o.host.contains(e)) {
      if (!inspecting() && !o.alwaysSelect) return;
      var pick = +e.dataset.k;
      if (ev.altKey) {
        var stk = stackAt(ev.clientX, ev.clientY).filter(function (x) { return vis(x, R); });
        if (stk.length > 1) {
          var idx = stk.findIndex(function (x) { return x.k === sel; });
          pick = stk[(idx + 1) % stk.length].k;
        }
      }
      V.select(pick); if (o.onClickCanvas) o.onClickCanvas(R.m.C[pick]);
      ev.stopPropagation(); return;
    }
    var tf = o.tree && ev.target.closest('[data-form]');
    if (tf && o.tree.contains(tf)) { V.selectForm(); ev.stopPropagation(); return; }
    var ta = o.tree && ev.target.closest('[data-a]');
    if (ta && o.tree.contains(ta)) { V.selectAttr(+ta.dataset.a); ev.stopPropagation(); return; }
    var tcol = o.tree && ev.target.closest('[data-col]');
    if (tcol && o.tree.contains(tcol)) { V.selectCol(+tcol.dataset.col); ev.stopPropagation(); return; }
    var trow = o.tree && ev.target.closest('[data-row]');
    if (trow && o.tree.contains(trow)) { V.selectRow(+trow.dataset.row); ev.stopPropagation(); return; }
    var tb = o.tree && ev.target.closest('[data-b]');
    if (tb && o.tree.contains(tb)) {
      var bk4 = +tb.dataset.b, b4 = R.m.B && R.m.B[bk4];
      V.selectButton(bk4);
      if (b4 && b4.kids && b4.kids.length) {
        var anc4 = o.host.querySelector('#b' + bk4);
        if (anc4) showSubmenuPopup(anc4, b4);
      }
      ev.stopPropagation(); return;
    }
    var t = o.tree && ev.target.closest('[data-k]');
    if (t && o.tree.contains(t)) { V.select(+t.dataset.k); ev.stopPropagation(); }
  });
  return V;
}

root.OneFormRender = { parse: parse, create: create, inspect: inspect, evs: evs, esc: esc, T: T, hl: hl, code: code };
})(window);
