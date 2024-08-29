use async_std::io::{self, WriteExt};

use clap::Parser;
use lettre::message::header::{self, ContentType, To};
use lettre::message::Mailboxes;
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::response::Response;
use lettre::transport::smtp::Error;
use lettre::{AsyncSmtpTransport, AsyncStd1Executor, AsyncTransport, Message};
use async_channel::bounded;

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

    /// Threads
    #[arg(short, long, default_value = "10")]
    threads: u32,

    /// DNS exfiltration payload
    #[arg(short, long)]
    exfil: String,
}

async fn send_email(
    recipients: To,
    from: String,
    subject: String,
    body: String,
    mailer: AsyncSmtpTransport<AsyncStd1Executor>,
) -> Result<Response, Error> {
    let email = Message::builder()
        .from(from.parse().unwrap())
        .reply_to(from.parse().unwrap())
        .mailbox(recipients)
        .subject(subject)
        .header(ContentType::TEXT_HTML)
        .body(body)
        .unwrap();

    // Send the email
    mailer.send(email).await
}

enum ProcessMessage {
    Email {
        recipients: To,
        from: String,
        subject: String,
        body: String,
    },
    Done,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
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

    let mailer: AsyncSmtpTransport<AsyncStd1Executor> =
        AsyncSmtpTransport::<AsyncStd1Executor>::starttls_relay(&args.server)
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
    let (send, recv) = bounded::<ProcessMessage>(args.threads as usize);
    let mut receivers = Vec::new();
    // start receiver threads
    for _ in 0..args.threads {
        let recv = recv.clone();
        let mailer = mailer.clone();
        let handle = tokio::spawn(async move {
            loop {
                match recv.recv().await {
                    Ok(ProcessMessage::Email {
                        recipients,
                        from,
                        subject,
                        body,
                    }) => {
                        send_email(recipients, from, subject, body, mailer.clone()).await.ok();
                    }
                    Ok(ProcessMessage::Done) => break,
                    Err(_) => break,
                }
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
            io::stdout().flush().await.unwrap();
            send.send(ProcessMessage::Email {
                recipients: to_header,
                from: args.from.as_ref().unwrap().to_owned(),
                subject: args.subject.clone(),
                body,
            }).await.unwrap();
            batch_recipients = String::new();
        }
        ilast = i;
    }

    if !batch_recipients.is_empty() {
        batch_recipients.pop();
        batch_recipients.pop();

        let mailboxes: Mailboxes = batch_recipients.parse().unwrap();
        let to_header: header::To = mailboxes.into();
        let body = args.exfil.replace(
            "FUZZ",
            format!(
                "{}",
                ilast.to_string()
            )
            .as_str(),
        );
        print!("\rSending email: [{}/{}]\n", ilast, match direction {
            1 => args.fuzz_end,
            -1 => args.fuzz_start,
            _ => 0,
        });
        send_email(
            to_header,
            args.from.as_ref().unwrap().to_owned(),
            args.subject.clone(),
            body,
            mailer.clone(),
        )
        .await?;
    }

    // close all the threads
    for _ in 0..args.threads {
        send.send(ProcessMessage::Done).await.unwrap();
    }
    for handle in receivers {
        handle.await.ok();
    }

    Ok(())
}
