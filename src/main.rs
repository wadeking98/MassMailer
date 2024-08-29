use std::io::{self, Write};

use clap::Parser;
use crossbeam::channel::bounded;
use lettre::message::header::{self, ContentType, To};
use lettre::message::Mailboxes;
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::response::Response;
use lettre::{Message, SmtpTransport, Transport};
use std::thread;

#[macro_use]
extern crate error_chain;

mod errors {
    // Create the Error, ErrorKind, ResultExt, and Result types
    error_chain! {}
}

use errors::*;

/// Parse smtp config from command line
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// SMTP username
    #[arg(short, long)]
    username: String,

    /// SMTP password
    #[arg(short, long)]
    password: String,

    /// SMTP server
    #[arg(short, long)]
    server: String,

    /// Email recipient
    #[arg(short, long)]
    recipient: String,

    /// Email sender
    #[arg(short, long, default_value = None)]
    from: Option<String>,

    /// Email subject
    #[arg(long, default_value = "DNS Test")]
    subject: String,

    /// Fuzz start
    #[arg(long, default_value = "0")]
    fuzz_start: u32,

    /// Fuzz end
    #[arg(long, default_value = "100")]
    fuzz_end: u32,

    /// Batch size
    #[arg(long, default_value = "2")]
    batch_size: u32,

    /// Per Email delay
    #[arg(long, default_value = "0.0")]
    delay: f32,

    /// Threads
    #[arg(short, long, default_value = "10")]
    threads: u32,

    /// DNS exfiltration payload
    #[arg(short, long)]
    exfil: String,
}

fn send_email(
    recipients: To,
    from: String,
    subject: String,
    body: String,
    mailer: &SmtpTransport,
) -> Result<Response> {
    let email = Message::builder()
        .from(from.parse().unwrap())
        .reply_to(from.parse().unwrap())
        .mailbox(recipients)
        .subject(subject)
        .header(ContentType::TEXT_HTML)
        .body(body)
        .chain_err(|| "Could not build email")?;

    // Send the email
    mailer.send(&email).chain_err(|| "Could not send email")
}

struct EmailMessage {
    recipients: To,
    from: String,
    subject: String,
    body: String,
}

fn main() -> Result<()> {
    let mut args = Args::parse();
    if args.from.is_none() {
        args.from = Some(args.username.clone());
    }

    if !args.recipient.contains("FUZZ") {
        panic!("Recipient must contain FUZZ")
    }
    if !args.exfil.contains("FUZZ") {
        panic!("Exfil must contain FUZZ")
    }

    let creds = Credentials::new(args.username, args.password);

    let mailer = SmtpTransport::starttls_relay(&args.server)
        .unwrap()
        .credentials(creds)
        .build();

    let mut batch_recipients = String::new();
    let mut ilast = 0;
    let mut range: Vec<u32> = (args.fuzz_start..=args.fuzz_end).collect();
    let mut direction = 1;
    if args.fuzz_end < args.fuzz_start {
        range = (args.fuzz_end..=args.fuzz_start).rev().collect();
        direction = -1;
    }
    let (send, recv) = bounded::<EmailMessage>(args.threads as usize);
    let mut receivers = Vec::new();
    // start receiver threads
    for _ in 0..args.threads {
        let recv = recv.clone();
        let mailer = mailer.clone();
        let handle = thread::spawn(move || loop {
            match recv.recv() {
                Ok(EmailMessage {
                    recipients,
                    from,
                    subject,
                    body,
                }) => {
                    send_email(recipients, from, subject, body, &mailer).ok();
                }
                Err(_) => break,
            }
        });
        receivers.push(handle);
    }
    for i in range {
        batch_recipients
            .push_str(format!("{}, ", &args.recipient.replace("FUZZ", &i.to_string())).as_str());
        if i % args.batch_size == 0 {
            batch_recipients.pop();
            batch_recipients.pop();
            let mailboxes: Mailboxes = batch_recipients.parse().unwrap();
            let to_header: header::To = mailboxes.into();
            let body = args.exfil.replace("FUZZ", &i.to_string());
            print!(
                "\rSending email: [{}/{}]",
                i,
                match direction {
                    1 => args.fuzz_end,
                    -1 => args.fuzz_start,
                    _ => 0,
                }
            );
            io::stdout().flush().unwrap();
            if args.delay > 0.0 {
                thread::sleep(std::time::Duration::from_millis((args.delay * 1000.0) as u64));
            }
            send.send(EmailMessage {
                recipients: to_header,
                from: args.from.as_ref().unwrap().to_owned(),
                subject: args.subject.clone(),
                body,
            })
            .chain_err(|| "Could not send email over channel")?;
            batch_recipients = String::new();
        }
        ilast = i;
    }

    if !batch_recipients.is_empty() {
        batch_recipients.pop();
        batch_recipients.pop();

        let mailboxes: Mailboxes = batch_recipients.parse().unwrap();
        let to_header: header::To = mailboxes.into();
        let body = args
            .exfil
            .replace("FUZZ", format!("{}", ilast.to_string()).as_str());
        print!(
            "\rSending email: [{}/{}]\n",
            ilast,
            match direction {
                1 => args.fuzz_end,
                -1 => args.fuzz_start,
                _ => 0,
            }
        );
        send.send(EmailMessage {
            recipients: to_header,
            from: args.from.as_ref().unwrap().to_owned(),
            subject: args.subject.clone(),
            body,
        })
        .chain_err(|| "Could not send email over channel")?;
    }

    // close all the threads
    drop(send);
    for handle in receivers {
        handle.join().unwrap();
    }

    Ok(())
}
