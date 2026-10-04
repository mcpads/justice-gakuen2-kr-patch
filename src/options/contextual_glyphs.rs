use super::model::OptionsFontRole;

pub(super) const OPTIONS_CONTROLLER_ICON_CODES: [u16; 8] = [
    0x02a0, 0x02a1, 0x02a2, 0x02a3, 0x02a4, 0x02a5, 0x02a6, 0x02a7,
];

pub(super) const SHARED_SOURCE_GRAPHIC_CODES: [u16; 12] = [
    0x0323, 0x0333, 0x033b, 0x0341, 0x0347, 0x034b, 0x0353, 0x0354, 0x0356, 0x0357, 0x035a, 0x035b,
];

pub(super) const PRESERVED_SOURCE_GRAPHIC_CODES: [u16; 20] = preserved_source_graphic_codes();

const fn preserved_source_graphic_codes() -> [u16; 20] {
    let mut codes = [0; 20];
    let mut index = 0;
    while index < OPTIONS_CONTROLLER_ICON_CODES.len() {
        codes[index] = OPTIONS_CONTROLLER_ICON_CODES[index];
        index += 1;
    }
    let mut shared_index = 0;
    while shared_index < SHARED_SOURCE_GRAPHIC_CODES.len() {
        codes[index + shared_index] = SHARED_SOURCE_GRAPHIC_CODES[shared_index];
        shared_index += 1;
    }
    codes
}

pub(super) const fn is_records_role(role: OptionsFontRole) -> bool {
    matches!(
        role,
        OptionsFontRole::RecordsMainHeading
            | OptionsFontRole::RecordsMainItem
            | OptionsFontRole::RecordsPrompt
            | OptionsFontRole::RecordsStatusHeading
            | OptionsFontRole::RecordsStatusMessage
    )
}
