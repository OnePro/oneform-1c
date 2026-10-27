#!/bin/bash
# Partial load of changed objects into the test infobase (skill db-load-xml) and UpdateDBCfg.
# usage: load.sh <configDir> <listfile> <logtag>
#   configDir  configuration XML export that contains the packed object
#   listfile   one object per line (e.g. Catalogs/Имя/Forms/Форма.xml), UTF-8 WITH BOM, CRLF
#   logtag     log goes to <logtag>.log, the last lines are printed
# env: ONEFORM_BUH_DB (required), ONEFORM_1CV8_BIN (directory of 1cv8.exe, default 8.3.27.2342),
#      ONEFORM_SKILLS (default ~/.agents/skills)
: "${ONEFORM_BUH_DB:?set ONEFORM_BUH_DB to the test infobase directory}"
BIN="${ONEFORM_1CV8_BIN:-C:\\Program Files\\1cv8\\8.3.27.2342\\bin}"
SK="${ONEFORM_SKILLS:-$HOME/.agents/skills}"
powershell.exe -NoProfile -File "$(cygpath -w "$SK/db-load-xml/scripts/db-load-xml.ps1")" -V8Path "$BIN" \
  -InfoBasePath "$ONEFORM_BUH_DB" -UserName admin -ConfigDir "$(cygpath -w "$1")" -Mode Partial \
  -ListFile "$(cygpath -w "$2")" -UpdateDB > "$3.log" 2>&1
tail -15 "$3.log"
