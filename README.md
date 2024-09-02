# MassMailer
This is a project for sending mass emails very quickly. I used it on a recent bug bounty project for enumerating valid usernames.
This will only really work if you have your own SMTP server setup, if you try and use this script through gmail or outlook you'll
probably get blocked immediatley.  
  
## Considerations
Make sure your SMTP mail server is up to the task, moniter the mail queue while running and make sure it doesn't fill up too much.
I found around 10 threads works best if your SMTP server is in the same region as the IMAP server you're sending to.
I was able to send about 300 emails/second using this script.

## Usage:
```
Usage: MassMailer [OPTIONS] --username <USERNAME> --password <PASSWORD> --server <SERVER> --recipient <RECIPIENT> --exfil <EXFIL>

Options:
  -u, --username <USERNAME>      SMTP username
  -p, --password <PASSWORD>      SMTP password
  -s, --server <SERVER>          SMTP server
  -r, --recipient <RECIPIENT>    Email recipient
  -f, --from <FROM>              Email sender
      --subject <SUBJECT>        Email subject [default: "DNS Test"]
      --fuzz-start <FUZZ_START>  Fuzz start [default: 0]
      --fuzz-end <FUZZ_END>      Fuzz end [default: 100]
      --batch-size <BATCH_SIZE>  Batch size [default: 2]
      --delay <DELAY>            Per Email delay [default: 0.0]
  -t, --threads <THREADS>        Threads [default: 10]
  -e, --exfil <EXFIL>            DNS exfiltration payload
  -h, --help                     Print help
  -V, --version                  Print version
```

This script assumes you can enumarate valid address based on if you get a DNS callback on valid addresses. The script doesn't monitor the DNS queries though,
you'll have to do that yourself through burpsuite collaborator or something.
The script accepts the FUZZ keyword in the mail recipient and the exfiltration payload. The exfiltration payload is usually just a link pointing to the
DNS that you control, something like this: `<a href='https://FUZZ.mydomain.com'></a>`.  
  
The batch-size parameter referes to the number of recipients in the `To:` email header, I set it for 2 here because I found unless the valid email address
was at the start or end of the `To:` header then I wouldn't get a callback, you may have a different experience.