extern crate maildir;

use maildir::MailEntry;
use maildir::Maildir;
use std::path::PathBuf;

fn unwrap_mail(mail: ::std::io::Result<MailEntry>) -> MailEntry {
    mail.unwrap_or_else(|e| {
        eprintln!("Error: {:?}", e);
        ::std::process::exit(1);
    })
}

fn list_mail(mail: MailEntry) {
    println!("Path:         {}", mail.path().display());
    println!("ID:           {}", mail.id());
    println!("Flags:        {}", mail.flags());
    println!("is_draft:     {}", mail.is_draft());
    println!("is_flagged:   {}", mail.is_flagged());
    println!("is_passed:    {}", mail.is_passed());
    println!("is_replied:   {}", mail.is_replied());
    println!("is_seen:      {}", mail.is_seen());
    println!("is_trashed:   {}", mail.is_trashed());
}

fn main() {
    // not sure whether this is actually fast or something, but we don't care here, do we?
    ::std::env::args()
        .skip(1)
        .map(PathBuf::from)
        .map(Maildir::from)
        .for_each(|mdir| {
            mdir.list_new().map(unwrap_mail).for_each(list_mail);

            mdir.list_cur().map(unwrap_mail).for_each(list_mail);
        });
}
