//! Order of the definitions in a BSL module: «Определения процедур и функций должны размещаться перед операторами
//! тела модуля». A minimal scanner (comments, strings, preprocessor lines, `&Directives`, `Перем` declarations,
//! `Процедура`/`Функция` ... `Конец...` blocks); `pack` refuses an edit that introduces a procedure after the body.

use serde_json::Value;

use crate::errors::{ExtractionError, Result, FORM_EDIT_REJECTED};

/// A procedure or function that follows a statement of the module body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Misplaced {
    /// 1-based line of the `Процедура`/`Функция` keyword.
    pub line: usize,
    pub name: String,
    /// 1-based line of the first statement of the module body.
    pub body_line: usize,
}

/// Code of each line with comments and string contents removed (`None` for a line without code: blank, comment,
/// `#...` preprocessor line, `&Directive` line, continuation of a multi-line string).
fn code_lines(text: &str) -> Vec<Option<String>> {
    let mut out = Vec::new();
    let mut in_string = false;
    for line in text.lines() {
        let mut code = String::new();
        let mut chars = line.chars().peekable();
        if in_string {
            // `|` starts a continuation line of a multi-line string
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }
            if chars.peek() == Some(&'|') {
                chars.next();
            }
        }
        while let Some(c) = chars.next() {
            if in_string {
                if c == '"' {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                    } else {
                        in_string = false;
                    }
                }
                continue;
            }
            match c {
                '/' if chars.peek() == Some(&'/') => break,
                '"' => {
                    in_string = true;
                    code.push('"');
                }
                _ => code.push(c),
            }
        }
        let trimmed = code.trim();
        let skip = trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('&');
        out.push(if skip { None } else { Some(trimmed.to_string()) });
    }
    out
}

/// Identifiers (lower case) at the start of `code`, up to the first character that is not part of a word or a blank.
fn words(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in code.chars() {
        if c.is_alphanumeric() || c == '_' {
            cur.push(c);
        } else {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur).to_lowercase());
            }
            if !c.is_whitespace() {
                break;
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur.to_lowercase());
    }
    out
}

const START: [&str; 4] = ["процедура", "функция", "procedure", "function"];
const END: [&str; 4] = ["конецпроцедуры", "конецфункции", "endprocedure", "endfunction"];
const VAR: [&str; 2] = ["перем", "var"];
const ASYNC: [&str; 2] = ["асинх", "async"];

/// The procedures and functions that are defined after a statement of the module body, in order.
pub fn misplaced_definitions(text: &str) -> Vec<Misplaced> {
    let mut found = Vec::new();
    let mut in_proc = false;
    let mut body_line: Option<usize> = None;
    for (i, code) in code_lines(text).into_iter().enumerate() {
        let Some(code) = code else { continue };
        let w = words(&code);
        let first = w.first().map(String::as_str).unwrap_or("");
        if in_proc {
            if END.contains(&first) {
                in_proc = false;
            }
            continue;
        }
        let at = usize::from(ASYNC.contains(&first) && w.get(1).is_some_and(|s| START.contains(&s.as_str())));
        let head = w.get(at).map(String::as_str).unwrap_or("");
        if START.contains(&head) {
            if let Some(body) = body_line {
                let name = code
                    .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .filter(|s| !s.is_empty())
                    .nth(at + 1)
                    .unwrap_or("")
                    .to_string();
                found.push(Misplaced { line: i + 1, name, body_line: body });
            }
            in_proc = true;
        } else if VAR.contains(&first) || END.contains(&first) {
            // module variables precede the procedures; a stray end keyword is not a statement
        } else if body_line.is_none() {
            body_line = Some(i + 1);
        }
    }
    found
}

/// Refuses an edited module that has a procedure after the body when the original module had none.
pub fn check_module_edit(path: &str, original: Option<&[u8]>, edited: &[u8]) -> Result<()> {
    let after = misplaced_definitions(&String::from_utf8_lossy(edited));
    let Some(first) = after.first() else { return Ok(()) };
    let before = original.map(|o| misplaced_definitions(&String::from_utf8_lossy(o))).unwrap_or_default();
    if !before.is_empty() {
        return Ok(());
    }
    Err(ExtractionError::new(
        FORM_EDIT_REJECTED,
        format!(
            "{path}: line {}: '{}' is defined after the statements of the module body (the first one is at line {}); 1C does not compile it («Определения процедур и функций должны размещаться перед операторами тела модуля»): put the procedure before the section «ОПЕРАТОРЫ ОСНОВНОЙ ПРОГРАММЫ» / before the first top-level statement",
            first.line, first.name, first.body_line
        ),
    )
    .with("kind", "bsl-procedure-after-body")
    .with("path", path)
    .with("line", Value::from(first.line)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = "\
Перем Глобальная;

#Область Служебные
// комментарий
&НаКлиенте
Процедура Первая() Экспорт
	Сообщить(\"Конец; Процедура Х\"); // Функция Y
КонецПроцедуры
#КонецОбласти

Функция Вторая(А,
	Б)
	Возврат А + Б;
КонецФункции

#Область ОПЕРАТОРЫ_ОСНОВНОЙ_ПРОГРАММЫ
Глобальная = 1;
Если Глобальная = 1 Тогда
	Глобальная = 2;
КонецЕсли;
#КонецОбласти
";

    #[test]
    fn a_legal_module_with_a_body_at_the_end_is_clean() {
        assert!(misplaced_definitions(BODY).is_empty());
        assert!(check_module_edit("FormModule.bsl", None, BODY.as_bytes()).is_ok());
    }

    #[test]
    fn a_procedure_after_the_body_is_found_with_its_line() {
        let text = format!("{BODY}\nПроцедура Новая()\n\tВозврат;\nКонецПроцедуры\n");
        let found = misplaced_definitions(&text);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].name, "Новая");
        assert_eq!(found[0].body_line, 17);
        assert_eq!(found[0].line, text.lines().position(|l| l.starts_with("Процедура Новая")).unwrap() + 1);
    }

    #[test]
    fn the_edit_is_refused_only_when_it_introduces_the_violation() {
        let edited = format!("{BODY}\nAsync Function New()\n\tReturn 1;\nEndFunction\n");
        let err = check_module_edit("Forms/Форма/FormModule.bsl", Some(BODY.as_bytes()), edited.as_bytes()).unwrap_err();
        assert_eq!(err.params["kind"], "bsl-procedure-after-body");
        assert!(err.message.contains("line 23") && err.message.contains("New") && err.message.contains("line 17"), "{}", err.message);
        // the original already had the violation: not the edit's fault
        assert!(check_module_edit("m", Some(edited.as_bytes()), format!("{edited}// x\n").as_bytes()).is_ok());
    }

    #[test]
    fn a_procedure_appended_to_a_module_without_a_body_is_fine() {
        let text = "Перем А;\nПроцедура П()\nКонецПроцедуры\n\nПроцедура Н()\n\tА = 1;\nКонецПроцедуры\n";
        assert!(misplaced_definitions(text).is_empty());
        assert!(misplaced_definitions("").is_empty());
    }

    #[test]
    fn variables_comments_directives_and_strings_are_not_statements() {
        let text = "// только комментарий\n#Если Сервер Тогда\n&НаСервере\nПерем Б;\nПроцедура П()\n\tТ = \"многострочная\n\t|Процедура Х\n\t|строка\";\nКонецПроцедуры\n#КонецЕсли\nПерем В;\nПроцедура Н()\nКонецПроцедуры\n";
        assert!(misplaced_definitions(text).is_empty(), "{:?}", misplaced_definitions(text));
    }
}
