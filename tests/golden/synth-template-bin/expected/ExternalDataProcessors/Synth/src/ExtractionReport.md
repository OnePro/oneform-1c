# Extraction report

- Status: `partial-reference`
- Code complete: yes
- Source: "Proc.epf" (sha256 `ac840e1875a04d3331f873c73871ca806aaed446a88d31a7fc72033cf1623fae`)
- Processor: "Obj" (`710723d8-83e7-48ae-9996-7a45571ed0df`)

## Counts

- Object module: extracted
- Forms: 1
- Templates: 1
- Warnings: 2

## Forms

- `Form_47200e71_ce18_488c_9d4a_169dd6066884` "Form" (`47200e71-ce18-488c-9d4a-169dd6066884`): module extracted, layout unsupported (basic)

## Templates

- `Template_68b792c0_6443_4ad3_86cf_adadc7d25135` "Tmpl" (`68b792c0-6443-4ad3-86cf-adadc7d25135`): Unknown, reference-only

## Warnings

- `FORM_PROJECTION_UNSUPPORTED` in `Forms/47200e71-ce18-488c-9d4a-169dd6066884`: {"alias": "Form_47200e71_ce18_488c_9d4a_169dd6066884", "backend": "basic"}
- `TEMPLATE_LAYOUT_UNSUPPORTED` in `Templates/68b792c0-6443-4ad3-86cf-adadc7d25135`: {"templateTypeCode": 3}

## Notes

- Form layouts are reference-only: the basic backend does not decode controls, attributes or events. The raw form stream is kept as binary evidence.
- The EPF is the single source of truth. `src/` is a derived projection for review; change the processor, save the EPF and extract again.
