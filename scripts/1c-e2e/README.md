# End-to-end check in real 1C (Windows, interactive session)

Runs an edited ordinary-form processor in 1C:Enterprise and compares the runtime properties of every control
with the original: only the intended differences may appear.

1. `oneform extract` the original EPF, edit `Form.xml` (`e2e_edit.py` is an example edit script).
2. `oneform pack`, then `epf_dump.py <epf> <tag>` opens the forms in 1C and writes a property dump
   (needs the launcher processor `target/dump-ws/dump.epf` and a test infobase, see the paths at the top of the script).
3. `dump_diff.py <original dump> <new dump> post` lists missing, extra and changed properties.

`e2e_variant.sh <tag> <E1,E2,..>` does steps 1-3 for a set of the example edits. The dump launcher is a 1C
data processor not stored here.
