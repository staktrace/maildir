use maildir::*;

use std::fs;
use std::path::*;

use mailparse::MailHeaderMap;

// `cargo package` doesn't package files with colons in the
// name, so we have to resort to naming it something else
// and renaming for the tests. Talk about ugly!
fn setup() {
    fs::rename(
        "testdata/maildir1/cur/1463868505.38518452d49213cb409aa1db32f53184_2_S",
        "testdata/maildir1/cur/1463868505.38518452d49213cb409aa1db32f53184:2,S",
    )
    .unwrap();
}

fn teardown() {
    fs::rename(
        "testdata/maildir1/cur/1463868505.38518452d49213cb409aa1db32f53184:2,S",
        "testdata/maildir1/cur/1463868505.38518452d49213cb409aa1db32f53184_2_S",
    )
    .unwrap();
}

#[test]
fn maildir_count() {
    setup();
    let maildir = Maildir::from("testdata/maildir1");
    assert_eq!(maildir.count_cur(), 1);
    assert_eq!(maildir.count_new(), 1);
    teardown();
}

#[test]
fn maildir_list() {
    setup();
    let maildir = Maildir::from("testdata/maildir1");
    let mut iter = maildir.list_new();
    let mut first = iter.next().unwrap().unwrap();
    assert_eq!(first.id(), "1463941010.5f7fa6dd4922c183dc457d033deee9d7");
    assert_eq!(
        first.headers().unwrap().get_first_value("Subject").unwrap(),
        Some(String::from("test"))
    );
    assert_eq!(first.is_seen(), false);
    let second = iter.next();
    assert!(second.is_none());

    let mut iter = maildir.list_cur();
    let mut first = iter.next().unwrap().unwrap();
    assert_eq!(first.id(), "1463868505.38518452d49213cb409aa1db32f53184");
    assert_eq!(
        first
            .parsed()
            .unwrap()
            .headers
            .get_first_value("Subject")
            .unwrap(),
        Some(String::from("test"))
    );
    assert_eq!(first.is_seen(), true);
    let second = iter.next();
    assert!(second.is_none());
    teardown();
}

#[test]
fn maildir_find() {
    setup();
    let maildir = Maildir::from("testdata/maildir1");
    assert_eq!(maildir.find("bad_id").is_some(), false);
    assert_eq!(
        maildir
            .find("1463941010.5f7fa6dd4922c183dc457d033deee9d7")
            .is_some(),
        true
    );
    assert_eq!(
        maildir
            .find("1463868505.38518452d49213cb409aa1db32f53184")
            .is_some(),
        true
    );
    teardown();
}

#[test]
fn mark_read() {
    setup();
    let maildir = Maildir::from("testdata/maildir1");
    assert_eq!(
        maildir
            .move_new_to_cur("1463941010.5f7fa6dd4922c183dc457d033deee9d7")
            .unwrap(),
        ()
    );
    // Reset the filesystem
    fs::rename(
        "testdata/maildir1/cur/1463941010.5f7fa6dd4922c183dc457d033deee9d7:2,",
        "testdata/maildir1/new/1463941010.5f7fa6dd4922c183dc457d033deee9d7",
    )
    .unwrap();
    teardown();
}

#[test]
fn check_received() {
    setup();
    let maildir = Maildir::from("testdata/maildir1");
    let mut iter = maildir.list_cur();
    let mut first = iter.next().unwrap().unwrap();
    assert_eq!(first.received().unwrap(), 1_463_868_507);
    teardown();
}

#[test]
fn check_create_dirs() {
    let maildir = Maildir::from("testdata/maildir2");
    assert!(!Path::new("testdata/maildir2").exists());
    assert!(!Path::new("testdata/maildir2/cur").exists());
    assert!(!Path::new("testdata/maildir2/new").exists());
    assert!(!Path::new("testdata/maildir2/tmp").exists());

    maildir.create_dirs().unwrap();
    assert!(Path::new("testdata/maildir2").exists());
    assert!(Path::new("testdata/maildir2/cur").exists());
    assert!(Path::new("testdata/maildir2/new").exists());
    assert!(Path::new("testdata/maildir2/tmp").exists());

    fs::remove_dir_all("testdata/maildir2").unwrap();
}

const TEST_MAIL_BODY: &[u8] = b"Return-Path: <of82ecuq@cip.cs.fau.de>
iginal-To: of82ecuq@cip.cs.fau.de
vered-To: of82ecuq@cip.cs.fau.de
ived: from faui0fl.informatik.uni-erlangen.de (unknown [IPv6:2001:638:a000:4160:131:188:60:117])
    by faui03.informatik.uni-erlangen.de (Postfix) with ESMTP id 466C1240A3D
    for <of82ecuq@cip.cs.fau.de>; Fri, 12 May 2017 10:09:45 +0000 (UTC)
ived: by faui0fl.informatik.uni-erlangen.de (Postfix, from userid 303135)
    id 389CC10E1A32; Fri, 12 May 2017 12:09:45 +0200 (CEST)
of82ecuq@cip.cs.fau.de
-Version: 1.0
ent-Type: text/plain; charset=\"UTF-8\"
ent-Transfer-Encoding: 8bit
age-Id: <20170512100945.389CC10E1A32@faui0fl.informatik.uni-erlangen.de>
: Fri, 12 May 2017 12:09:45 +0200 (CEST)
: of82ecuq@cip.cs.fau.de (Johannes Schilling)
ect: maildir delivery test mail

y is Boomtime, the 59th day of Discord in the YOLD 3183";

#[test]
fn check_store_new() {
    let maildir = Maildir::from("testdata/maildir2");
    maildir.create_dirs().unwrap();

    assert_eq!(maildir.count_new(), 0);
    maildir.store_new(TEST_MAIL_BODY).unwrap();
    assert_eq!(maildir.count_new(), 1);

    fs::remove_dir_all("testdata/maildir2").unwrap();
}

#[test]
fn check_store_cur() {
    let maildir = Maildir::from("testdata/maildir2");
    maildir.create_dirs().unwrap();
    let testflags = "FRS";

    assert_eq!(maildir.count_cur(), 0);
    maildir
        .store_cur_with_flags(TEST_MAIL_BODY, testflags)
        .unwrap();
    assert_eq!(maildir.count_cur(), 1);

    let mut iter = maildir.list_cur();
    let first = iter.next().unwrap().unwrap();
    assert_eq!(first.flags(), testflags);

    fs::remove_dir_all("testdata/maildir2").unwrap();
}
