# physis-core Expansion Plan

## 1. Domain & Mode Expansion

The base 5×14 semiotic grid (HEAL/CONSTRUCT/FABRICATE/BOND/STUDY × 14 modes) is intentionally a **base layer**. New dimensions are added as **additive packs**, not by bloating the grid.

### Missing Domains (to be added as ontology packs)
- `GOVERN` — rule, law, adjudicate, allocate
- `EXCHANGE` — trade, buy/sell, negotiate value
- `EXPRESS` — communicate, signal, art-as-language
- `DEFEND` — secure, protect, pre-empt harm
- `MEANING` — ritual, worship, transcend, commemorate
- `EXPLORE` — navigate, orient, discover the unknown

### Missing Modes (to be added as ontology pack extensions)
- `DECIDE` / `WILL` — agency & intention
- `REASON` — deliberate, infer
- `REMEMBER` — recall, archive
- `MEASURE` — assess, diagnose
- `TEACH` — impart
- `PERSUADE` — negotiate, convince
- `IMAGINE` — envision pre-creation
- `SHARE` — give, distribute
- `PROTEST` — resist, rebuke
- `CELEBRATE` — mourn, ritualize affect

**Expansion approach:**
- Each new domain/mode pair is released as an **ontology JSON pack** that consumers drop into `config/`.
- The grid stays at 5×14; new cells are reachable via **facets** (`sub_domain`, `sub_mode`) or by selecting an axis-kind pack (`office`, `machine`, `agent` already supported).
- Pack authors add entries with `domain: "GOVERN"` and `mode: "DECIDE"` etc., and users can classify against them.

### Proposed Canonical Ordering (view hint, not constraint)

Domains:
```
HEAL < BOND < STUDY < CONSTRUCT < FABRICATE < GOVERN < EXCHANGE < EXPRESS < DEFEND < MEANING < EXPLORE
```

Modes (expanded):
```
REST < SENSE < REMEMBER < LIFT < MOVE < WORK < DECIDE < REASON < CREATE < IMAGINE < TEACH < GUIDE < PLAY < BRAINSTORM < MAINTAIN < PLAN < PROTEST < CELEBRATE
```

---

## 2. Ontology Pack Structure

Each ontology pack is a JSON file placed in `physis-core/config/`. Packs follow the same schema as built-in ontologies:

```json
{
  "name": "Governance",
  "category": "civic",
  "domain": "GOVERN",
  "mode": "DECIDE",
  "axis_kind": "legal",
  "unit": "cases",
  "hints": ["statute", "precedent", "appeal"],
  "facets": {
    "sub_domain": "litigation",
    "sub_mode": "adjudicate",
    "lifecycle": "Operate",
    "agency": "Automated",
    "scale": "Organizational",
    "abstraction": "Concrete"
  }
}
```

### Pack Naming Convention
- File name: `<domain>_<mode>_ontology.json`, e.g., `govern_decide_ontology.json`
- Pack may contain many entries; each entry follows `OntologyEntry` schema.

### Adding a Pack
1. Create JSON file with domain/mode values outside the original 5×14 grid.
2. Ensure `facets` are used for any extra dimensions.
3. Drop the file in `physis-core/config/`.
4. The `OntologyLoader` automatically loads it via `include_str!` or `load_from_str` at runtime.
5. Users can then classify text and get hits in the new domain/mode cell, with facet filtering via `FacetFilter`.

### Example Pack: `govern_decide_ontology.json`
```json
{
  "domains": [
    {
      "name": "Court Ruling",
      "category": "legal",
      "domain": "GOVERN",
      "mode": "DECIDE",
      "axis_kind": "legal",
      "unit": "cases",
      "hints": ["judgment", "summary", "order"],
      "facets": {
        "lifecycle": "Operate",
        "agency": "Automated"
      }
    },
    {
      "name": "Contract Signing",
      "category": "legal",
      "domain": "GOVERN",
      "mode": "SIGN",
      "axis_kind": "legal",
      "unit": "agreements",
      "hints": ["signature", "parties", "seal"],
      "facets": {
        "lifecycle": "Build",
        "agency": "Self"
      }
    }
  ]
}
```

The pack can be loaded with:
```rust
let loader = OntologyLoader::load_from_str(&include_str!("../config/govern_decide_ontology.json")).unwrap();
```

---

## 3. Getting Started Guide

See `GETTING_STARTED.md` for a step‑by‑step guide.

---

## 4. Stripe License Selling Guide

See `STRIPE_LICENSE_GUIDE.md` for integrating Stripe to sell physis-core licenses.

---

## 5. Pricing & Advertisement Ideas

### Pricing Tiers

| Tier | Price (monthly) | Features |
|------|-----------------|----------|
| **Indie** | $29 / mo | Single‑user license, deterministic embeddings, full CLI + studio access, standard ontology (33 domains). |
| **Pro** | $99 / mo | Single‑user + ONNX embedder support (`--features embed-onnx`), priority ontology pack updates, seat‑limited (up to 3 seats), quality‑tracker export/import. |
| **Enterprise** | $299 / mo | Unlimited seats, custom ontology packs, dedicated support, on‑premises deployment, API access for internal tooling, SLA on uptime. |

*All tiers include a 14‑day free trial (via Stripe trial mode). Discounts available for annual subscriptions (2 months free).*

### Advertisement Channels

1. **Rust Community** – post on r/rust, r/programming, Rust subreddit, Rust Lang weekly newsletter; highlight the zero‑model AI aspect.
2. **AI/ML Newsletters** – AI Weekly, The Sequence, Synced Review; emphasize deterministic embeddings for reproducible research.
3. **Product‑Ops & SRE Forums** – discuss incident classification, feedback loops; target audience for SRE/incident‑management tools.
4. **Design & UX Blogs** – articles on semiotic grids for sense‑making; target designers exploring mental‑model mapping.
5. **Content Marketing** – write a 3‑part series: (a) "Why deterministic embeddings matter", (b) "How the semiotic grid structures knowledge", (c) "Monetizing with Stripe: a developer's guide".
6. **Twitter / X Threads** – short demos: `physis-core classify "..."` with screenshots of the studio heatmap.
7. **Conference Sponsorship** – small booth or lightning talk at Rust Conf, Strange Loop, or AI/ML meet‑ups.
8. **Partner Integrations** – offer a simple SDK for Node.js/Python that wraps physis-core classification; cross‑promote.

### Referral Program

- Give existing customers a unique referral code.
- Upon a successful purchase by a new user, both get one month free (or a 20% discount on renewal).
- Track referrals via Stripe's metadata or your own database.

### Conversion Funnel

1. **Awareness** – tweet/demo + link to GETTING_STARTED.md.
2. **Interest** – visitor reads getting‑started, tries classification.
3. **Consideration** – opens studio, experiments with ontology packs.
4. **Decision** – clicks "Buy License" (Stripe Checkout).
5. **Retention** – periodic email with new packs, quality‑tracker tips, feature updates.

---
## 6. Marketing Plan (executed after the free‑sample launch)

### 6.1 Positioning & Tagline
- **Tagline:** “Deterministic semi‑grid classification, zero‑model AI, embed anywhere.”
- **Core message:** Offline, explainable, learn‑from‑feedback classification without third‑party APIs.

### 6.2 Target Segments (priority)
| Segment | Pain point | Why physis‑core |
|---|---|---|
| SRE / Incident response | Log‑snippet overload, need feedback loop | Offline, penalty/boost learning, no data export |
| Technical writers / Doc‑ops | Auto‑tag thousands of specs, need explainable tags | 5×14 grid + ontology packs, deterministic |
| Rust systems developers | Want tiny embeddable “brain”, no GPU/runtime | Model‑free random‑projection, compile‑anywhere |
| Product‑ops / SaaS founders | Need issue‑routing/tagging, avoid per‑token costs | Flat‑fee licence, unlimited calls |

### 6.3 Launch funnel
| Phase | Goal | Tactics | Owner | ETA |
|---|---|---|---|---|
| **Pre‑launch buzz** | Build waiting list & early interest | • Teaser tweet thread (deterministic AI) <br>• Posts on r/rust, r/programming <br>• Blog: “Why deterministic embeddings matter” | Founder/Marketing | Week 1‑2 |
| **Free‑sample release** | Get hands‑in‑the‑door usage | • Publish download at `praxisweb.xyz/physis/free-sample` <br>• “7‑day trial” CTA <br>• GitHub Discussions for Q&A | Founder | Week 3 |
| **Beta‑program** | Collect feedback, refine packs & UI | • Invite 10‑15 users (SRE, doc‑ops, Rust devs) <br>• Offer discounted annual licence for feedback <br>• Weekly sync calls | PM / Community lead | Week 4‑6 |
| **Commercial launch** | Convert trial → paid licence | • Stripe product page (Indie/Pro/Enterprise) <br>• Email drip “trial ending” + upgrade incentive <br>• LinkedIn / Twitter ads targeting SRE & dev‑tool handles | Founder / Sales | Week 7‑8 |
| **Post‑launch growth** | Expand reach, add packs | • Release first ontology pack (e.g., `govern_decide`) <br>• Partner with SRE tooling companies <br>• SEO for “semiotic grid classification” | Community lead | Month 2‑3 |

### 6.4 Messaging framework
| Message | Audience | Channel |
|---|---|---|
| “Run AI on your laptop, no GPU, no internet.” | Rust developers, SRE | Twitter, Reddit, Hacker News |
| “Classify support tickets, then teach the system from your mistakes.” | SRE, incident‑management | Blogs, webinars, conference talks |
| “Tag your technical docs with a 5×14 domain‑mode grid, no model files.” | Technical writers | Newsletters, Medium, Dev.to |
| “Predictable $29‑$299/mo licence – unlimited classifications.” | SaaS founders, product‑ops | Email outreach, LinkedIn ads |

### 6.5 Metrics to track
- **Download count** of the free sample.
- **Trial‑to‑paid conversion rate** (target 5‑10 %).
- **Churn** after first year.
- **Ontology‑pack adoption** (how many users add custom packs).
- **Community contributions** (new packs, bug reports).

### 6.6 Budget (first 3 months)
| Item | Cost (USD) |
|---|---|
| Paid ads (Twitter/Linke​In) | $200 |
| Conference sponsorship (small booth) | $500 |
| Design of trial key graphic | $150 |
| **Total** | **≈ $850** (plus variable Stripe fees) |

### 6.7 Success criteria (by month 3)
- ≥ 500 downloads of the free sample.
- ≥ 30 trial users who upgrade to a paid licence.
- At least one community‑contributed ontology pack merged into the repo.
- Positive feedback NPS > 30.

---
*This plan builds on the pricing, advertisement, and referral concepts already captured in `PLANNING.md`. All tasks are deliberately small enough to be completed in a single work‑day, allowing rapid iteration and early revenue.*