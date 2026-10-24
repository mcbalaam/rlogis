use super::*;

#[test]
fn parses_entries_and_continuations() {
    let log = LogFile::parse(concat!(
        "[2026-10-17 17:23:55.085] DEBUG: World loaded!\n",
        "[2026-10-17 17:24:02.500] SAY: user/(Name) (mob_1) \"Hello\" (bar (10,20,2))\n",
        " - Using: /datum/orbit_menu [0x2100161c]\n",
        " - Action: orbit {\"ref\":\"[mob_3356]\"}\n",
        "[2026-10-17 17:24:03.000] Starting up round ID 9452.\n",
    ));

    assert_eq!(log.rows.len(), 5);
    assert_eq!(log.entry_count, 3);
    assert_eq!(log.rows[0].line_no, 1);
    assert_eq!(log.rows[0].time.unwrap().to_string(), "17:23:55.085");
    assert_eq!(log.rows[0].category, Category::Debug);
    assert_eq!(log.rows[0].message, "World loaded!");

    assert_eq!(log.rows[1].category, Category::Say);
    assert_eq!(log.rows[1].time.unwrap().hour, 17);
    assert_eq!(log.rows[1].badges.len(), 1);
    assert_eq!(log.keys[0].key, "user");
    assert_eq!(log.keys[0].ckey, "user");

    assert!(log.rows[2].continuation);
    assert_eq!(log.rows[2].category, Category::Say);
    assert_eq!(log.rows[2].message, "Using: /datum/orbit_menu [0x2100161c]");
    assert_eq!(log.rows[2].line_no, 3);

    assert!(log.rows[4].time.is_some());
    assert_eq!(log.rows[4].category, Category::Other);
    assert_eq!(log.rows[4].message, "Starting up round ID 9452.");
}

#[test]
fn binds_key_and_ckey() {
    let log = LogFile::parse(concat!(
        "[2026-10-17 17:24:02.500] OOC: Fenix Armory/(Fenix Armory) \"hi\"\n",
        "[2026-10-17 17:24:03.500] SAY: fenixarmory/(Fenix Armory) \"hello\"\n",
    ));

    assert_eq!(log.keys.len(), 1);
    assert_eq!(log.keys[0].key, "Fenix Armory");
    assert_eq!(log.keys[0].ckey, "fenixarmory");
    assert_eq!(log.rows[0].badges[0].key, 0);
    assert_eq!(log.rows[1].badges[0].key, 0);

    let name = log.rows[0].badges[0].name.clone().unwrap();
    assert_eq!(&log.rows[0].message[name], "Fenix Armory");
}

#[test]
fn extracts_multiple_badges_and_keeps_no_key() {
    let log = LogFile::parse(concat!(
        "[2026-10-17 17:24:02.500] ATTACK: Karavill/(Name) attacked victim_1/(Victim)\n",
        "[2026-10-17 17:24:03.500] SAY: *no key*/(Poly) \"Squawk\"\n",
    ));

    let keys: Vec<&str> = log.keys.iter().map(|player| player.ckey.as_str()).collect();
    assert_eq!(keys, ["karavill", "victim1", "*no key*"]);
    assert_eq!(log.rows[0].badges.len(), 2);
    assert_eq!(log.rows[0].badges[0].key, 0);
    assert_eq!(log.rows[0].badges[1].key, 1);

    let pseudo_badge = &log.rows[1].badges[0];
    let pseudo = &log.keys[pseudo_badge.key as usize];
    assert!(pseudo.system);
    assert_eq!(pseudo.ckey, "*no key*");
    assert_eq!(
        &log.rows[1].message[pseudo_badge.key_start..pseudo_badge.key_end],
        "*no key*"
    );

    let second_name = log.rows[0].badges[1].name.clone().unwrap();
    assert_eq!(&log.rows[0].message[second_name], "Victim");
}

#[test]
fn formats_html_player_tags() {
    let log = LogFile::parse(concat!(
        "[2026-10-17 17:33:27.631] ADMIN: <a href='byond://?priv_msg=mrsnake'>Mrsnake</a>/(Магуро Хамати) начал отвечать на тикет #4.\n",
        "[2026-10-17 17:33:28.000] SAY: Fenix Armory/(Vladislav Vorobyov) said &#34;hi&#34; &amp; left.\n",
    ));

    let formatted = log.rows[0].formatted.as_ref().unwrap();
    assert_eq!(
        formatted.text,
        "mrsnake/(Магуро Хамати) начал отвечать на тикет #4."
    );
    assert_eq!(formatted.badges.len(), 1);
    let player = &log.keys[formatted.badges[0].key as usize];
    assert_eq!(player.ckey, "mrsnake");
    let name = formatted.badges[0].name.clone().unwrap();
    assert_eq!(&formatted.text[name], "Магуро Хамати");

    let formatted = log.rows[1].formatted.as_ref().unwrap();
    assert_eq!(
        formatted.text,
        "Fenix Armory/(Vladislav Vorobyov) said \"hi\" & left."
    );
    assert_eq!(
        log.keys[formatted.badges[0].key as usize].ckey,
        "fenixarmory"
    );
}

#[test]
fn uses_priv_msg_from_html_for_ckey() {
    let log = LogFile::parse(
        "[2026-10-17 17:33:27.631] ADMIN: <a href='byond://?priv_msg=stupid1'>Stupid_1</a>/(Stupid_1) started.\n",
    );

    let formatted = log.rows[0].formatted.as_ref().unwrap();
    assert_eq!(formatted.text, "stupid1/(Stupid_1) started.");
    let player = &log.keys[formatted.badges[0].key as usize];
    assert_eq!(player.ckey, "stupid1");
    assert!(player.matches_name("Stupid_1"));
}

#[test]
fn raw_badges_skip_html_tag_text() {
    let log = LogFile::parse(
        "[2026-10-17 17:33:27.631] ADMIN: <a href='byond://?priv_msg=mrsnake'>Mrsnake</a>/(Магуро Хамати) started.\n",
    );
    assert!(log.rows[0].badges.is_empty());
    assert!(!log.keys.iter().any(|player| player.ckey == "a"));
}

#[test]
fn matches_key_with_duplicate_mob_names() {
    let log = LogFile::parse(concat!(
        "[2026-10-17 17:25:26.451] OOC: maleK1337/(Malek1337) \"tr\"\n",
        "[2026-10-17 17:24:02.500] OOC: Fenix Armory/(Fenix Armory) \"hi\"\n",
        "[2026-10-17 17:24:03.500] SAY: mrsnake/(Магуро Хамати) \"hello\"\n",
    ));

    let male = &log.keys[log.rows[0].badges[0].key as usize];
    assert!(male.matches_name("Malek1337"));

    let fenix = &log.keys[log.rows[1].badges[0].key as usize];
    assert_eq!(fenix.ckey, "fenixarmory");
    assert!(fenix.matches_name("Fenix Armory"));

    let mrsnake = &log.keys[log.rows[2].badges[0].key as usize];
    assert!(!mrsnake.matches_name("Магуро Хамати"));
}

#[test]
fn finds_mob_and_location_marks() {
    let log = LogFile::parse(
        "[2026-10-17 17:30:01.917] ATTACK: *no key*/(goldgrub) (mob_491) has eaten the plasma ore! (Laser Tag Arena (84,109,2))\n",
    );
    let row = &log.rows[0];

    let mob = row
        .marks
        .iter()
        .find(|mark| matches!(mark.kind, MarkKind::Mob))
        .unwrap();
    assert_eq!(&row.message[mob.start..mob.end], "mob_491");

    let (full, name, coords) = row
        .marks
        .iter()
        .find_map(|mark| match &mark.kind {
            MarkKind::Location { name, coords } => Some((mark, name.clone(), coords.clone())),
            MarkKind::Mob
            | MarkKind::AsCharacter { .. }
            | MarkKind::Character { .. }
            | MarkKind::Frequency { .. }
            | MarkKind::AdminPm { .. }
            | MarkKind::TicketRef { .. }
            | MarkKind::Sensitive { .. } => None,
        })
        .unwrap();
    assert_eq!(
        &row.message[full.start..full.end],
        "(Laser Tag Arena (84,109,2))"
    );
    assert_eq!(&row.message[name], "Laser Tag Arena");
    assert_eq!(&row.message[coords], "84,109,2");
}

#[test]
fn finds_marks_in_formatted_text() {
    let log = LogFile::parse(
        "[2026-10-17 17:33:27.631] ADMIN: <a href='byond://?priv_msg=mrsnake'>Mrsnake</a>/(Магуро Хамати) did (Laser Tag Arena (84,109,2))\n",
    );
    let formatted = log.rows[0].formatted.as_ref().unwrap();
    let (name, coords) = formatted
        .marks
        .iter()
        .find_map(|mark| match &mark.kind {
            MarkKind::Location { name, coords } => Some((name.clone(), coords.clone())),
            MarkKind::Mob
            | MarkKind::AsCharacter { .. }
            | MarkKind::Character { .. }
            | MarkKind::Frequency { .. }
            | MarkKind::AdminPm { .. }
            | MarkKind::TicketRef { .. }
            | MarkKind::Sensitive { .. } => None,
        })
        .unwrap();
    assert_eq!(&formatted.text[name], "Laser Tag Arena");
    assert_eq!(&formatted.text[coords], "84,109,2");
}

#[test]
fn computes_format_virtual_columns() {
    let log = LogFile::parse(concat!(
        "[2026-10-17 17:24:02.500] SAY: abc/(Short Name) (mob_1) \"hello\"\n",
        "[2026-10-17 17:24:03.500] SAY: longkey_123/(Very Long Name) (mob_12345) \"hi\"\n",
        "[2026-10-17 17:24:04.500] MISC: no player here\n",
    ));

    assert_eq!(log.format_columns.key_chars, "longkey123".len());
    assert_eq!(
        log.format_columns.name_chars,
        "Very Long Name".len() + 1 + "mob_12345".len()
    );
}

#[test]
fn formats_usr_client_lines() {
    let log = LogFile::parse(
        "[2026-10-17 17:23:57.542] MISC: Golub_lesnoi (usr:Максим Думкопф[(149,133,3)]) : commandbar_typing=1\n",
    );
    let row = &log.rows[0];
    let mark = row
        .marks
        .iter()
        .find_map(|mark| match &mark.kind {
            MarkKind::AsCharacter {
                key,
                key_end,
                name,
                coords,
            } => Some((*key, *key_end, name.clone(), coords.clone())),
            _ => None,
        })
        .unwrap();

    assert_eq!(log.keys[mark.0 as usize].ckey, "golublesnoi");
    assert_eq!(&row.message[..mark.1], "Golub_lesnoi");
    assert_eq!(&row.message[mark.2], "Максим Думкопф");
    assert_eq!(&row.message[mark.3], "149,133,3");
}

#[test]
fn finds_radio_frequencies() {
    let log = LogFile::parse(
        "[2026-10-17 17:33:34.268] SAY: dogusu/(Анастасия Гудкова) (mob_3381) [145.9] (spans:  command_headset ) \"yo\" (brig (10,20,2))\n",
    );
    let row = &log.rows[0];

    let frequency = row
        .marks
        .iter()
        .find_map(|mark| match &mark.kind {
            MarkKind::Frequency { value, spans } => Some((value.clone(), spans.clone())),
            _ => None,
        })
        .expect("frequency mark");

    assert_eq!(&row.message[frequency.0], "145.9");
    assert_eq!(
        frequency
            .1
            .as_ref()
            .map(|range| &row.message[range.clone()]),
        Some(" (spans:  command_headset )")
    );
}

#[test]
fn parses_dead_chat_key_suffix() {
    let log = LogFile::parse(concat!(
        "[2026-10-17 17:38:15.643] GAME: Dorman[DC]/(Dorman) (mob_3421) is no longer owning mob\n",
        "[2026-10-17 18:10:33.662] ATTACK: @Mrsnake[DC]/(Магуро Хамати) grabbed someone\n",
    ));

    assert_eq!(log.keys[0].ckey, "dorman");
    assert_eq!(log.keys[0].key, "Dorman");
    assert_eq!(
        &log.rows[0].message[log.rows[0].badges[0].key_start..log.rows[0].badges[0].key_end],
        "Dorman[DC]"
    );

    assert_eq!(log.keys[1].ckey, "mrsnake");
    assert_eq!(log.keys[1].key, "Mrsnake");
    assert_eq!(
        &log.rows[1].message[log.rows[1].badges[0].key_start..log.rows[1].badges[0].key_end],
        "@Mrsnake[DC]"
    );
    let name = log.rows[1].badges[0].name.clone().unwrap();
    assert_eq!(&log.rows[1].message[name], "Магуро Хамати");
}

#[test]
fn handles_crlf_and_empty_input() {
    let log = LogFile::parse("[2026-10-17 17:23:55.085] GAME: one\r\n - detail\r\n");
    assert_eq!(log.rows.len(), 2);
    assert_eq!(log.rows[0].message, "one");
    assert_eq!(log.rows[1].message, "detail");

    assert!(LogFile::parse("").rows.is_empty());
}

#[test]
fn unknown_category_is_preserved_as_message() {
    let log = LogFile::parse("[2026-10-17 17:23:55.085] WAT: something\n");
    assert_eq!(log.rows[0].category, Category::Other);
    assert_eq!(log.rows[0].message, "something");
}

#[test]
fn time_range_is_tracked() {
    let log =
        LogFile::parse("[2026-10-17 17:23:55.085] MISC: a\n[2026-10-17 18:23:55.999] MISC: b\n");
    let (first, last) = log.time_range().unwrap();
    assert_eq!(first.to_string(), "17:23:55.085");
    assert_eq!(last.to_string(), "18:23:55.999");
}
