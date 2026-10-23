# Extraction report

- Status: `partial-reference`
- Code complete: yes
- Source: "donor.epf" (sha256 `d832f5a53069b6da226b4f92617b67f11d58901ecfbf2eb5730e4c86a23ac82b`)
- Processor: "ВнешняяОбработка1" (`c5912057-dc42-417c-bc2e-758a81c5642f`)

## Counts

- Object module: empty
- Forms: 1
- Templates: 4
- Warnings: 5

## Forms

- `MainForm` "Форма" (`0a4d3811-c3ab-4e8b-b826-6144d8606e9d`): module extracted, layout unsupported (basic)

## Templates

- `MainTemplate` "Макет" (`2b556694-58ba-41a0-84fe-7ea8313365ae`): Unknown, reference-only
- `Template2` "Макет2" (`5c9d9f39-6dea-46b0-b5dd-d241d1da0366`): Unknown, reference-only
- `Template_7937f297_7b71_4830_8fa5_91e8ba586b36` "Макет1" (`7937f297-7b71-4830-8fa5-91e8ba586b36`): Unknown, reference-only
- `Template_dc09fc43_3dc7_48ad_b69b_2681503366c6` "Макет3" (`dc09fc43-3dc7-48ad-b69b-2681503366c6`): Unknown, reference-only

## Warnings

- `FORM_PROJECTION_UNSUPPORTED` in `Forms/0a4d3811-c3ab-4e8b-b826-6144d8606e9d`: {"alias": "MainForm", "backend": "basic"}
- `TEMPLATE_KIND_UNKNOWN` in `Templates/2b556694-58ba-41a0-84fe-7ea8313365ae`: {"templateTypeCode": 0}
- `TEMPLATE_KIND_UNKNOWN` in `Templates/5c9d9f39-6dea-46b0-b5dd-d241d1da0366`: {"templateTypeCode": 6}
- `TEMPLATE_KIND_UNKNOWN` in `Templates/7937f297-7b71-4830-8fa5-91e8ba586b36`: {"templateTypeCode": 4}
- `TEMPLATE_KIND_UNKNOWN` in `Templates/dc09fc43-3dc7-48ad-b69b-2681503366c6`: {"templateTypeCode": 1}

## Notes

- Form layouts are reference-only: the basic backend does not decode controls, attributes or events. The raw form stream is kept as binary evidence.
- The EPF is the single source of truth. `src/` is a derived projection for review; change the processor, save the EPF and extract again.
