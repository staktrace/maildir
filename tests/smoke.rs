use maildir::*;

use std::borrow::Cow;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;

use mailparse::MailHeaderMap;
use percent_encoding::percent_decode;
use tempfile::tempdir;
use walkdir::WalkDir;

static TESTDATA_DIR: &str = "testdata";

// `cargo package` doesn't package files with certain characters, such as
// colons, in the name, so we percent-decode the file names when copying the
// data for the tests.
fn with_maildir<F>(name: &str, func: F)
where
    F: FnOnce(Maildir),
{
    let tmp_dir = tempdir().expect("could not create temporary directory");
    let tmp_path = tmp_dir.path();
    for entry in WalkDir::new(TESTDATA_DIR) {
        let entry = entry.expect("directory walk error");
        let relative = entry.path().strip_prefix(TESTDATA_DIR).unwrap();
        if relative.parent().is_none() {
            continue;
        }
        let decoded_bytes: Cow<[u8]> = percent_decode(relative.as_os_str().as_bytes()).into();
        let decoded = OsStr::from_bytes(&decoded_bytes);
        if entry.path().is_dir() {
            fs::create_dir(tmp_path.join(decoded)).expect("could not create directory");
        } else {
            fs::copy(entry.path(), tmp_path.join(decoded)).expect("could not copy test data");
        }
    }
    func(Maildir::from(tmp_path.join(name)));
}

fn with_maildir_empty<F>(name: &str, func: F)
where
    F: FnOnce(Maildir),
{
    let tmp_dir = tempdir().expect("could not create temporary directory");
    let tmp_path = tmp_dir.path();
    func(Maildir::from(tmp_path.join(name)));
}

#[test]
fn maildir_count() {
    with_maildir("maildir1", |maildir| {
        assert_eq!(maildir.count_cur(), 1);
        assert_eq!(maildir.count_new(), 1);
    });
}

#[test]
fn maildir_list() {
    with_maildir("maildir1", |maildir| {
        let mut iter = maildir.list_new();
        let mut first = iter.next().unwrap().unwrap();
        assert_eq!(first.id(), "1463941010.5f7fa6dd4922c183dc457d033deee9d7");
        assert_eq!(
            first.headers().unwrap().get_first_value("Subject"),
            Some(String::from("test"))
        );
        assert_eq!(first.is_seen(), false);
        let second = iter.next();
        assert!(second.is_none());

        let mut iter = maildir.list_cur();
        let mut first = iter.next().unwrap().unwrap();
        assert_eq!(first.id(), "1463868505.38518452d49213cb409aa1db32f53184");
        assert_eq!(
            first.parsed().unwrap().headers.get_first_value("Subject"),
            Some(String::from("test"))
        );
        assert_eq!(first.is_seen(), true);
        let second = iter.next();
        assert!(second.is_none());
    })
}

#[test]
fn maildir_list_subdirs() {
    with_maildir("maildir2", |maildir| {
        let subdirs: Vec<_> = maildir
            .list_subdirs()
            .map(|dir| {
                dir.unwrap()
                    .path()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        assert_eq!(2, subdirs.len());
        assert!(subdirs.contains(&".Subdir1".into()));
        assert!(subdirs.contains(&".Subdir2".into()));
        assert!(!subdirs.contains(&"..Subdir3".into()));
    });
}

#[test]
fn maildir_find() {
    with_maildir("maildir1", |maildir| {
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
    })
}

#[test]
fn check_delete() {
    with_maildir("maildir1", |maildir| {
        assert_eq!(
            maildir
                .find("1463941010.5f7fa6dd4922c183dc457d033deee9d7")
                .is_some(),
            true
        );
        assert_eq!(
            maildir
                .delete("1463941010.5f7fa6dd4922c183dc457d033deee9d7")
                .is_ok(),
            true
        );
        assert_eq!(
            maildir
                .find("1463941010.5f7fa6dd4922c183dc457d033deee9d7")
                .is_some(),
            false
        );
    })
}

#[test]
fn check_copy_and_move() {
    with_maildir("maildir1", |maildir1| {
        with_maildir("maildir2", |maildir2| {
            let id = "1463868505.38518452d49213cb409aa1db32f53184";

            // check that we cannot copy a message from and to the same maildir
            assert_eq!(
                maildir1.copy_to(id, &maildir1).unwrap_err().kind(),
                std::io::ErrorKind::InvalidInput,
            );

            // check that the message is present in maildir1 but not in maildir2
            assert!(maildir1.find(id).is_some());
            assert!(maildir2.find(id).is_none());
            // also check that the failed self-copy a few lines up didn't corrupt the
            // message file.
            assert!(maildir1.find(id).unwrap().date().is_ok());

            // copy the message from maildir1 to maildir2
            maildir1.copy_to(id, &maildir2).unwrap();

            // check that the message is now present in both
            assert!(maildir1.find(id).is_some());
            assert!(maildir2.find(id).is_some());

            // move the message from maildir2 to maildir1
            maildir2.move_to(id, &maildir1).unwrap();

            // check that the message is now only present in maildir1
            assert!(maildir1.find(id).is_some());
            assert!(maildir2.find(id).is_none());
        })
    })
}

#[test]
fn mark_read() {
    with_maildir("maildir1", |maildir| {
        assert_eq!(
            maildir
                .move_new_to_cur("1463941010.5f7fa6dd4922c183dc457d033deee9d7")
                .unwrap(),
            ()
        );
    });
}

#[test]
fn check_received() {
    with_maildir("maildir1", |maildir| {
        let mut iter = maildir.list_cur();
        let mut first = iter.next().unwrap().unwrap();
        assert_eq!(first.received().unwrap(), 1_463_868_507);
    });
}

#[test]
fn check_create_dirs() {
    with_maildir_empty("maildir2", |maildir| {
        assert!(!maildir.path().exists());
        for name in &["cur", "new", "tmp"] {
            assert!(!maildir.path().join(name).exists());
        }

        maildir.create_dirs().unwrap();
        assert!(maildir.path().exists());
        for name in &["cur", "new", "tmp"] {
            assert!(maildir.path().join(name).exists());
        }
    });
}

const TEST_MAIL_BODY: &[u8] = b"Return-Path: <of82ecuq@cip.cs.fau.de>
X-Original-To: of82ecuq@cip.cs.fau.de
Delivered-To: of82ecuq@cip.cs.fau.de
Received: from faui0fl.informatik.uni-erlangen.de (unknown [IPv6:2001:638:a000:4160:131:188:60:117])
        by faui03.informatik.uni-erlangen.de (Postfix) with ESMTP id 466C1240A3D
        for <of82ecuq@cip.cs.fau.de>; Fri, 12 May 2017 10:09:45 +0000 (UTC)
Received: by faui0fl.informatik.uni-erlangen.de (Postfix, from userid 303135)
        id 389CC10E1A32; Fri, 12 May 2017 12:09:45 +0200 (CEST)
To: of82ecuq@cip.cs.fau.de
MIME-Version: 1.0
Content-Type: text/plain; charset=\"UTF-8\"
Content-Transfer-Encoding: 8bit
Message-Id: <20170512100945.389CC10E1A32@faui0fl.informatik.uni-erlangen.de>
Date: Fri, 12 May 2017 12:09:45 +0200 (CEST)
From: of82ecuq@cip.cs.fau.de (Johannes Schilling)
Subject: maildir delivery test mail

Today is Boomtime, the 59th day of Discord in the YOLD 3183";

#[test]
fn check_store_new() {
    with_maildir_empty("maildir2", |maildir| {
        maildir.create_dirs().unwrap();

        assert_eq!(maildir.count_new(), 0);
        let id = maildir.store_new(TEST_MAIL_BODY);
        assert!(id.is_ok());
        assert_eq!(maildir.count_new(), 1);

        let id = id.unwrap();
        let msg = maildir.find(&id);
        assert!(msg.is_some());

        assert_eq!(
            msg.unwrap().parsed().unwrap().get_body_raw().unwrap(),
            b"Today is Boomtime, the 59th day of Discord in the YOLD 3183".as_ref()
        );
    });
}

#[test]
fn check_store_cur() {
    with_maildir_empty("maildir2", |maildir| {
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
    });
}

#[test]
fn check_flag_fiddling() {
    with_maildir_empty("maildir2", |maildir| {
        maildir.create_dirs().unwrap();
        let id = maildir.store_cur_with_flags(TEST_MAIL_BODY, "SR").unwrap();

        assert_eq!(maildir.count_cur(), 1);
        assert_eq!(maildir.find(&id).unwrap().flags(), "RS");
        maildir.remove_flags(&id, "FS").unwrap();
        assert_eq!(maildir.find(&id).unwrap().flags(), "R");
        maildir.add_flags(&id, "RF").unwrap();
        assert_eq!(maildir.find(&id).unwrap().flags(), "FR");
        maildir.set_flags(&id, "SF").unwrap();
        assert_eq!(maildir.find(&id).unwrap().flags(), "FS");
    });
}
