# Physis Pro — Distribution & Mailing Lists (audiences that may want to hear about it)

> Companion to `PROSPECT_LIST_EXPANDED.csv` (direct customer outreach). This file is the
> BROADCAST layer: existing mailing lists / newsletters / launch venues whose subscribers
> already match physis-pro's two audiences — **users** (Rust devs, ML engineers, industrial
> software buyers) and **investors** (AI/industrial-tech VCs, angels).

## How to use
1. Ship `physis-core` to crates.io FIRST (hard prerequisite — memory: publishing needs it).
2. Then run launches in this order: crates.io + r/rust → TWiR → Show HN → Ben's Bites News →
   newsletters → investor lists.
3. Every venue below is free unless noted. No cold-buying of lists.

---

## A. Rust developer audience (users — crate users)

| # | Venue | What it is | How to get in | Why fit |
|---|-------|-----------|---------------|---------|
| 1 | **This Week in Rust** | THE weekly Rust newsletter (email) | Submit news via PR to github.com/rust-lang/this-week-in-rust; project/tooling updates must first be posted on r/rust for consideration | Exact audience for a new Rust engine/crate |
| 2 | **Crate of the Week** (in TWiR) | Weekly highlighted crate | Suggest + vote in the internals forum "Crate of the Week" thread | Free spotlight for physis-core/physis_pro |
| 3 | **r/rust** (~400k) | Reddit launch post ("I built X in Rust") | Post directly; TWiR editors pick from here | Primary launch venue; feeds TWiR |
| 4 | **rust-ml Zulip** | ML working group chat + biweekly open video calls | Join via github.com/rust-ml/wg | Direct ML-in-Rust community |
| 5 | **arewelearningyet.com** | Rust-ML ecosystem index | PR to the repo adding physis-pro | Where Rust ML people look first |
| 6 | **crates.io / docs.rs** | Registry listing | `cargo publish` → auto-listed; README quality drives discovery | Baseline presence (needs physis-core first) |

## B. AI/dev-tools launch platforms (users + investors watch these)

| # | Venue | What it is | How to get in | Why fit |
|---|-------|-----------|---------------|---------|
| 8 | **Hacker News — Show HN** | Front-page tech forum, investors read daily | news.ycombinator.com/submit, title "Show HN: ..." | Biggest single-day user+investor spike |
| 9 | **Ben's Bites News** | HN-style submission platform feeding a 120k-subscriber founder/investor newsletter | Submit at news.bensbites.co | Investors explicitly in audience |
| 10 | **TLDR AI (~1.1M readers)** | Daily AI newsletter | Pitch via TLDR site (tips@tldr.tech style) | Massive technical reach |
| 11 | **The Rundown AI (~2M)** | Largest dedicated AI newsletter, use-case driven | Pitch via rundown newsletter contact | Executives/investors read it |
| 12 | **Latent Space (200k+)** | AI-engineer newsletter+podcast | News tips / guest angle: offline air-gapped AI engines | Precise AI-engineer audience |
| 13 | **Import AI (136k+, Jack Clark)** | Research-oriented AI weekly | jack-clark.net subscribe/contact | Research credibility signal |

## C. Investor channels

| # | Channel | What it is | How to get in | Why fit |
|---|--------|-----------|---------------|---------|
| 14 | **Ben's Bites (main newsletter)** | Founder/investor lens, ~120k | Same submission pipeline as #9 | Investors explicitly named as audience by the newsletter itself |
| 15 | **AI Supremacy (Michael Spencer)** | Substack on AI business/investing | ai-supremacy.com, pitch guest essay or news tip | Investor/business-of-AI readership |
| 16 | **Italian angel/VC networks** | ItalianTech Angels, LITA, CDP Venture (Italia 4.0 fund) | Apply/pitch via their sites | Local fit: Verona-first doctrine, Italian SME vertical |
| 17 | **Confindustria newsletters (IdentitàMedia)** | Italian employers' association comms | confindustria.it newsletter signup | Reaches Italian manufacturing execs (customer-side) |

## D. Industrial/manufacturing audience (buyers)

| # | Venue | What it is | How to get in | Fit |
|---|-------|-----------|---------------|-----|
| 18 | **Confindustria Verona** newsletter/events | Verona-area manufacturers assoc. | Attend/submit via verona.confindustria.it; pair with Generalmeccanica follow-up | Highest-fit local broadcast (Verona-first doctrine) |
| 19 | **Il Sole 24 Ore Innovazione / ANSA Industry 4.0** | Italian industry press | PR pitch AFTER first pilot metrics exist — GO_TO_MARKET doc warns against claiming ROI before a pilot proves value | Broad Italian industry reach |

---

## Sequencing (first-euro doctrine)
1. `cargo publish` physis-core → physis_pro (prerequisite for everything).
2. r/rust launch post + Show HN same day.
3. TWiR news submission + Crate of the Week suggestion next cycle.
4. Ben's Bites News submission (investor visibility).
5. Newsletter pitches (TLDR AI, Rundown, Latent Space) with the sample shift report as proof.
6. Confindustria Verona outreach only after pilot metrics exist.

