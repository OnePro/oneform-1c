#!/bin/bash
# usage: load.sh <configDir> <listfile> <logtag>
powershell.exe -NoProfile -File "C:/Users/egor/.agents/skills/db-load-xml/scripts/db-load-xml.ps1" -V8Path "C:\Program Files\1cv8\8.3.27.2342\bin" -InfoBasePath "C:\Users\egor\Documents\1с\bases\dental_vlad_buh" -UserName admin -ConfigDir "$(cygpath -w "$1")" -Mode Partial -ListFile "$(cygpath -w "$2")" -UpdateDB > "$3.log" 2>&1
tail -15 "$3.log"
