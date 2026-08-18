# First-customer funnel and business email

This document records the current acquisition experiment for Physis. The
purpose is to sell a measurable operational result before selling a platform
subscription.

## Free sample package

Keep the Praxis homepage and login at `praxisweb.xyz/`. Publish the Physis
sales surface at `praxisweb.xyz/physis`; the sample package should become
`praxisweb.xyz/physis/sample` when the upload funnel is added. It should
contain:

1. an Italian one-page shift report;
2. the CSV/JSON input used to produce it;
3. a short explanation of the losses and recommended actions;
4. a screenshot or link to the Physis console;
5. a clear limitation statement: this is decision support, not a safety or
   maintenance guarantee;
6. the call to action: “Send one week of your own export.”

The existing sample is concrete enough for this package: plant OEE 78.5%, a
lathe at 67.8%, 45 minutes of breakdown, a 4.4σ temperature anomaly, and an
estimated 2h10m to the configured temperature limit. Source files:

- `docs/business/sample-shift-report.md`
- `docs/business/sample-shift-report.html`
- `docs/business/demo-shift.json`

## Upload sneak peek

The next public feature should be a deliberately narrow upload preview:

- accept CSV and JSON first; do not begin with arbitrary PDFs;
- cap the upload at 5 MB and one machine or one week;
- parse and validate locally where possible;
- redact or reject obvious personal data and credentials;
- process in an isolated, short-lived job;
- delete uploaded data and generated artifacts automatically;
- return only a preview: OEE, top loss, one anomaly, and one suggested action;
- require an email address before downloading the full report;
- show the retention policy beside the upload control.

The current console has working Decision Planner and Admin Review panels at the
technical workbench route `praxisweb.xyz/physis/workbench`, but does not yet
provide this public upload flow. Do not advertise customer-file uploads until
the retention, isolation, abuse limits, and email capture are implemented.

## What is tested and what is not

The revenue-critical industrial demo test checks:

- OEE and loss decomposition;
- a degrading-machine anomaly;
- remaining-useful-life projection;
- Italian report sections;
- concrete countermeasures.

The test proves that the engine can produce the sample output. It does not yet
prove monetary value. No real customer baseline has measured review hours,
avoided downtime, reduced scrap, or changed decisions.

For the first pilot, capture these values before delivery:

| Metric | Baseline | After two reports |
|---|---:|---:|
| Hours spent preparing the weekly review |  |  |
| Downtime hours in the selected scope |  |  |
| Scrap/rework cost |  |  |
| Repeat incidents |  |  |
| Actions completed before next shift |  |  |

Use this simple value calculation:

`estimated value = avoided downtime + avoided scrap + review hours saved`

Do not claim ROI from the sample alone. The sample demonstrates the shape of
the result; the paid pilot measures whether the result changes operations.

## First-customer offer

### 5-day Shift Loss Review

- input: one week of an existing CSV/JSON export;
- output: Italian report, loss Pareto, one anomaly review, and three actions;
- delivery: five business days plus a 30-minute review call;
- first-reference price: €1,500–€2,500 paid upfront;
- conversion: recurring monitoring only after two useful review cycles.

Lead with the result, not the implementation:

> Send one week of your existing production data. By Friday you will know
> where the shift lost time, which signal deserves attention, and what the
> next shift should verify.

## Prospect shortlist

These are public research targets, not contacted customers. Verify the contact
person and address before sending.

| Company | Fit | Public contact |
|---|---|---|
| BMech | Smaller CNC workshop; milling, turning, automation components | `info@bmech.it` |
| Officine Cioetto | 33 employees, 33 CNC machines, double-shift production | `info@officinecioetto.it` |
| DFM | CNC, aerospace/defence quality, MES/process integration | `info@dfm.srl` |
| FILA Srl | CNC/casting operation with production, quality, and admin workflows | `commerciale@filasrl.it` |
| OLMA | Industry 4.0/OEE-aware CNC manufacturer; possible channel partner | contact form |

Public sources:

- [BMech](https://www.bmech.it/en/)
- [Officine Cioetto](https://www.officinecioetto.it/en/)
- [DFM](https://www.dfm.srl/)
- [FILA contact page](https://www.filasrl.it/contact/)
- [OLMA](https://www.olmasrl.com/en/)

## Outreach drafts

### BMech

**Subject:** Un report gratuito su una settimana di produzione

Buongiorno,

ho visto che BMech gestisce lavorazioni CNC di fresatura e tornitura per
componenti di automazione.

Sto sviluppando Physis, uno strumento che trasforma un normale export di
produzione in un breve rapporto italiano con OEE, principali perdite, fermi,
anomalie e azioni da verificare nel turno successivo.

Posso prepararvi gratuitamente un esempio usando dati anonimi o un piccolo
export reale. Non serve installare nulla e non vi propongo un progetto MES.

Se mi inviate un CSV di una settimana, vi restituisco un’anteprima con una
pagina di risultato. Se non emerge nulla di utile, vi rimane comunque il
report.

Vi interessa provarlo?

Cordiali saluti,

[Nome] — [telefono] — https://praxisweb.xyz

### Officine Cioetto

**Subject:** Analisi gratuita dei fermi su un turno di produzione

Buongiorno,

ho visto che Officine Cioetto lavora su più reparti CNC e su turni produttivi.

Sto cercando una prima azienda italiana con cui validare un rapporto automatico
di fine turno. Il sistema legge dati già disponibili e mette in evidenza quale
macchina ha perso più tempo, quali cause si ripetono, dove compaiono scarti o
microfermate e quale anomalia merita una verifica.

Vi posso inviare un pacchetto dimostrativo in italiano oppure analizzare
gratuitamente un piccolo estratto dei vostri dati, senza collegamento ai vostri
sistemi e senza modificare nulla.

In cambio vi chiederei solo 30 minuti di feedback sul fatto che il rapporto sia
utile a produzione e manutenzione.

Posso mandarvi il campione?

Cordiali saluti,

[Nome] — [telefono] — https://praxisweb.xyz

### DFM

**Subject:** DFM + Physis: rapporto operativo sopra i dati MES

Buongiorno,

ho visto che DFM dispone già di processi CNC, qualità e gestione MES. Per
questo non vorrei proporvi un altro cruscotto.

Physis lavora sopra gli export esistenti e produce un rapporto leggibile per il
responsabile di produzione: perdite OEE, eventi anomali, segnali di deriva e
azioni da discutere nel turno successivo.

La proposta iniziale è semplice: ci date un export anonimizzato di una
settimana, vi restituiamo un rapporto italiano e misuriamo insieme se riduce il
tempo di revisione o migliora la scelta delle priorità.

Vi invio un esempio già pronto?

Cordiali saluti,

[Nome] — [telefono] — https://praxisweb.xyz

## Business email

`gio@praxisweb.xyz` is possible, but the domain’s DNS provider must be
controlled. The current website is served from the pansa-ts host
(`gio@100.65.59.18`, alias `pansa-ts` in `/home/gio/.zshrc`). The web server and
the mail provider are separate services: keep the existing web records and add
the mail records alongside them.

### Recommended quickest setup: Zoho Mail

Use this when a real mailbox that can send and receive as
`gio@praxisweb.xyz` is needed:

1. Create a Zoho Mail organization account.
2. Add and verify `praxisweb.xyz` using the TXT or CNAME record Zoho gives.
3. Add the user `gio`.
4. Replace/add the provider’s MX records at the domain DNS host.
5. Add the provider’s SPF and DKIM records.
6. Add a DMARC record after outbound mail is working.
7. Send a test from a different account and reply to it.

Zoho’s official setup requires domain verification, MX records for delivery,
and SPF/DKIM for deliverability. [Zoho custom-domain setup](https://www.zoho.com/mail/help/adminconsole/add-domains.html)

### Cheapest receive-only setup: Cloudflare Email Routing

If the immediate need is simply to receive mail at `gio@praxisweb.xyz` and
forward it to an existing inbox, Cloudflare Email Routing can do that without
a separate mailbox. It requires the domain to use Cloudflare DNS, a verified
destination address, and an MX/SPF setup. [Cloudflare Email Routing](https://developers.cloudflare.com/email-service/get-started/route-emails/)

This is not the best first choice for sales outreach because sending from the
custom address requires an SMTP/mailbox provider and correct authentication.

### Full productivity setup: Google Workspace

Use Google Workspace if Gmail, Calendar, Drive, and Meet are worth the monthly
cost. The setup wizard verifies domain ownership and provides the DNS records;
aliases such as `hello@`, `sales@`, and `support@` can then be added. [Google Workspace custom email](https://workspace.google.com/business/new-business/)

### Suggested addresses

- `gio@praxisweb.xyz` — personal founder identity;
- `hello@praxisweb.xyz` — public website and sample package;
- `sales@praxisweb.xyz` — prospect replies;
- `support@praxisweb.xyz` — customer issues.

Keep `physis@praxisweb.xyz` as an alias because existing product material uses
it as the API and support contact.

Start with one paid mailbox, `gio@praxisweb.xyz`, and aliases rather than
buying multiple users. Keep the current website DNS records intact; only add
the mail-provider verification, MX, SPF, DKIM, and later DMARC records.
