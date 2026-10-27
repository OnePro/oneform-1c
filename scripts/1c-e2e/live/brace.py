"""Minimal 1C brace-text parser. Node = list; atoms are str (quoted strings -> ('"',str) not needed: kept as str with leading \x00 marker)."""
def parse(text):
    i = 0; n = len(text)
    def val():
        nonlocal i
        # at '{' -> list; else atom up to , or }
        if text[i] == '{':
            i += 1; out = []
            while True:
                while i < n and text[i] in ' \r\n\t': i += 1
                if text[i] == '}':
                    i += 1; return out
                if text[i] == ',':
                    i += 1; continue
                out.append(val())
        if text[i] == '"':
            j = i + 1; s = []
            while True:
                if text[j] == '"':
                    if j + 1 < n and text[j+1] == '"':
                        s.append('"'); j += 2; continue
                    break
                s.append(text[j]); j += 1
            i = j + 1
            return "\x00" + "".join(s)
        j = i
        while text[j] not in ',}\r\n': j += 1
        s = text[i:j]; i = j
        return s.strip()
    while text[i] != '{': i += 1
    return val()

def is_node(x): return isinstance(x, list)
def dump(x):
    if isinstance(x, list): return "{" + ",".join(dump(c) for c in x) + "}"
    return '"' + x[1:] + '"' if x.startswith("\x00") else x
