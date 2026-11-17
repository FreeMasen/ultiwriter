use ultiwriter::mock::MockBox;
use ultiwriter::{CardKind, Client, Dialect, Error};

fn connect(m: &mut MockBox) -> Client<&mut MockBox> {
    Client::connect(m).expect("handshake")
}

#[test]
fn handshake_survives_dropped_first_command() {
    let mut m = MockBox::new();
    assert!(m.drop_next);
    let c = connect(&mut m);
    assert_eq!(c.dialect(), Dialect::Ascii);
}

#[test]
fn detect_and_identify() {
    let mut m = MockBox::new();
    let mut c = connect(&mut m);
    assert!(c.card_present().unwrap());
    let id = c.identify().unwrap();
    assert_eq!((id.manufacturer, id.device), (0x01, 0xD5));
    assert_eq!(id.size(), 0x10_0000);

    m.card_present = false;
    assert!(!connect(&mut m).card_present().unwrap());
}

#[test]
fn read_spans_chunks() {
    let mut m = MockBox::new();
    for (i, b) in m.flash.iter_mut().enumerate() {
        *b = (i * 7 % 251) as u8;
    }
    let expect = m.flash[100..100 + 9000].to_vec();
    let mut c = connect(&mut m);
    let mut buf = vec![0; 9000];
    c.read(CardKind::Viking, 100, &mut buf).unwrap();
    assert_eq!(buf, expect);
}

#[test]
fn write_pads_tail_and_skips_blank_pages() {
    let mut m = MockBox::new();
    let mut c = connect(&mut m);
    let mut data = vec![0xFF; 3 * 128];
    data[0] = 0x11;
    data.truncate(2 * 128 + 5); // pages: data, blank, short tail
    data[2 * 128] = 0x22;
    c.write_pages(CardKind::Viking, 0x1000, &data, true, |_| {})
        .unwrap();
    assert_eq!(m.pages_written, 2);
    assert_eq!(m.flash[0x1000], 0x11);
    assert_eq!(m.flash[0x1100], 0x22);
    assert!(m.flash[0x1101..0x1180].iter().all(|&b| b == 0xFF));
}

#[test]
fn erase_then_restore_roundtrip() {
    let mut m = MockBox::new();
    m.flash[..4].copy_from_slice(&[0, 0, 0, 0]); // previously programmed
    let image: Vec<u8> = (0..1000u32).map(|i| (i % 256) as u8).collect();
    let mut c = connect(&mut m);
    c.erase().unwrap();
    c.write(CardKind::Viking, 0, &image).unwrap();
    let mut back = vec![0; image.len()];
    c.read(CardKind::Viking, 0, &mut back).unwrap();
    assert_eq!(back, image);
    assert_eq!(m.erase_count, 1);
}

#[test]
fn unknown_command_reply_is_an_error() {
    let mut m = MockBox::new();
    let mut c = connect(&mut m);
    // Kind-4 reads use opcode 29, which an ASCII box rejects with "cmder".
    let mut buf = [0; 4];
    assert!(matches!(
        c.read(CardKind::Vikant, 0, &mut buf),
        Err(Error::BadReply { .. })
    ));
}
