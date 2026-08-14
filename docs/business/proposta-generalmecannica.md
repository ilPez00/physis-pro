# Physis — Motore Semiotico per il Backoffice Industriale

## Proposta per Generalmeccanica

---

## 1. Executive Summary

Generalmeccanica opera come **facilitatore di processo** — la vostra ragione d'essere è portare chiarezza, organizzazione e comunicazione efficace nei reparti produttivi dei vostri clienti.

Physis è un motore di intelligenza artificiale semiotica che classifica qualsiasi testo in una griglia universale di 70 celle (5 domini × 14 modi), rivelando la struttura nascosta delle attività umane e delle macchine. Per Generalmeccanica significa: **trasformare la vostra esperienza di facilitazione in un servizio scalabile, misurabile e vendibile.**

Questa proposta descrive come Physis può potenziare il vostro ruolo di facilitatori di processo — dalla diagnosi dei problemi di produzione al monitoraggio continuo della salute organizzativa.

---

## 2. Cos'è Physis

Physis non è un LLM generico né un tradizionale sistema di classificazione. È un **motore semiotico** che mappa qualsiasi comunicazione scritta — report di produzione, email, verbali di riunioni, log macchina, messaggi WhatsApp tra capi reparto — in una griglia semantica universale di 70 celle.

La griglia incrocia **5 domini** (cosa) con **14 modi** (come):

- **Domini:** FABRICATE (produrre), CONSTRUCT (costruire/organizzare), HEAL (curare/rigenerare), BOND (relazioni), STUDY (analizzare/conoscere).
- **Modi base:** CREATE (creare), WORK (eseguire), WALK (procedere), LIFT (sforzo di picco), LEARN (apprendere), REST (attesa/riposo).
- **Modi estesi:** DESTROY (dismettere/demolire), SENSE (rilevare/misurare), GUIDE (guidare/coordinare), PLAY (esplorare), BRAINSTORM (ideare) e — particolarmente rilevanti per l'industria — **MAINTAIN (manutenere)**, **MOVE (movimentare/logistica)**, **PLAN (pianificare la produzione)**.

I tre modi estesi industriali sono una differenza concreta per Generalmeccanica: manutenzione, movimentazione materiali e pianificazione non sono più ricondotti a celle generiche, ma riconosciuti come **modi nativi e distinti** — quindi smistabili e misurabili separatamente.

Una porzione della griglia (modi base, a titolo illustrativo):

| | **CREATE** | **WORK** | **WALK** | **MAINTAIN** | **MOVE** | **PLAN** |
|---|---|---|---|---|---|---|
| **FABRICATE** | Produrre | Amministrare | Distribuire | Manutenere | Movimentare | Pianificare |
| **CONSTRUCT** | Organizzare | Operare | Transitare | Riparare | Trasportare | Schedulare |
| **HEAL** | Rigenerare | Curare | Prevenire | Mantenere in forma | Spostare | Programmare |
| **BOND** | Unire | Collaborare | Incontrare | Coltivare | Connettere | Coordinare |
| **STUDY** | Sperimentare | Analizzare | Esplorare | Ripassare | Trasferire | Progettare |

Ogni cella della griglia contiene un **centroide semantico** calcolato da oltre 630 voci ontologiche distribuite su 10 ontologie (umano, macchina, semiotica, ufficio, agente AI, e altre), più 20 ontologie di dominio aggiuntive. Quando inviate un testo a Physis, il motore lo incorpora in uno spazio vettoriale a 768 dimensioni (BGE-base-en-v1.5) e calcola la similarità coseno con ogni cella, restituendo una classifica ordinata per punteggio.

### Le Macchine di Physis

Physis è composto da macchine specializzate, ognuna dedicata a un aspetto dell'attività umana. Per Generalmeccanica, cinque macchine sono particolarmente rilevanti:

| Macchina | Ruolo | Rilevanza per Generalmeccanica |
|---|---|---|
| **Polis** | Grafo sociale | Analisi delle relazioni tra reparti, rilevamento di attriti |
| **Elenchus** | Logica e deduzione | Validazione di processi decisionali, coerenza organizzativa |
| **Chronos** | Temporale | Sequenziamento attività, schedulazione, flussi temporali |
| **Lex** | Contratti e vincoli | Conformità, accordi di servizio, obblighi contrattuali |
| **Techne** | Produzione | Monitoraggio macchinari, qualità, efficienza produttiva |

---

## 3. Come Funziona

Il processo è lineare e richiede zero infrastruttura:

1. **Acquisizione** — Inviate un testo via API REST (curl, integrazione custom) o tramite la web app Praxis (praxisweb.xyz).
2. **Embedding** — Il testo viene convertito in un vettore a 768 dimensioni dal modello BGE-base-en-v1.5.
3. **Classificazione** — Il vettore viene confrontato con tutte le 70 celle della griglia semiotica. Ogni cella riceve un punteggio di similarità (da −1 a +1).
4. **Interpretazione** — I risultati rivelano la natura semiotica del testo: cosa sta realmente accadendo in quel processo, al di là delle parole usate.

### Perché funziona anche in Italiano

Le frasi italiane vengono proiettate nel medesimo spazio vettoriale delle ontologie, senza bisogno di traduzione. I numeri, misurati su un set di test curato e verificato a ogni rilascio:

- **Testi inglesi:** 98.2% di accuratezza sul dominio, 89.1% sulla cella esatta (su un set di test ampliato che include i nuovi modi industriali).
- **Testi italiani (frasi industriali e quotidiane):** 86.7% sul dominio e 86.7% sulla cella al primo tentativo — adatto allo smistamento automatico, con la cella corretta quasi sempre tra le prime tre.

L'accuratezza sull'italiano è un nostro cantiere attivo: il set di test italiano fa parte della suite di regressione, quindi ogni miglioramento è misurato, non dichiarato. Per i casi d'uso critici in italiano il workshop di onboarding configura ontologie con vocabolario specifico del vostro dominio, che alza sensibilmente la precisione sul vostro lessico reale.

---

## 4. Casi d'Uso per Generalmeccanica

### 4.1 Diagnostica dei Fermo Macchina

Generalmeccanica riceve segnalazioni di fermo macchina dai clienti. Physis classifica automaticamente ogni segnalazione, identificando se si tratta di un problema produttivo (FABRICATE×REST), manutentivo (HEAL×WORK), organizzativo (CONSTRUCT×WALK) o relazionale (BOND×WORK).

### 4.2 Allineamento dei Team di Produzione

I report di produzione, le email tra capi reparto e i verbali delle riunioni vengono analizzati per rilevare disallineamenti semiotici — per esempio, un team parla di "ottimizzazione" (STUDY×WORK) mentre l'altro parla di "esecuzione" (CONSTRUCT×WORK). Physis segnala la divergenza prima che diventi un problema.

### 4.3 Monitoraggio della Salute Organizzativa

La macchina Polis analizza la comunicazione tra reparti, misurando la reciprocità e la forza dei legami. Quando un rapporto si raffredda (bassa reciprocità), Physis lo segnala prima che la collaborazione si rompa.

### 4.4 Conformità e Contratti (Lex)

I contratti di servizio e gli accordi con i fornitori vengono analizzati dalla macchina Lex, che verifica la coerenza tra gli impegni presi e le attività registrate nei report di produzione.

### 4.5 OEE, Manutenzione Predittiva e Rapporto di Turno

Oltre alla classificazione, Physis calcola direttamente gli indicatori di stabilimento:

- **OEE per macchina e per impianto** — disponibilità × prestazione × qualità, con il dettaglio delle sei grandi perdite (guasti, setup, microfermate, velocità ridotta, scarti) ordinate per impatto.
- **Rilevamento anomalie** — ogni macchina ha una linea di base "sana" appresa dai dati; una lettura anomala viene segnalata con la sua distanza in sigma (es. "temperatura cuscinetto a 4.4σ dalla norma").
- **Vita utile residua (RUL)** — proiezione del tempo rimanente prima che una metrica in deriva raggiunga il limite (es. "limite 85°C raggiunto tra ~2 ore").
- **Contromisure proposte** — il motore di analisi rielabora le anomalie registrate e propone azioni correttive concrete.

Tutto confluisce nel **Rapporto di Turno**: un documento leggibile, in italiano, generato automaticamente alla fine di ogni turno — pronto da inviare via email o stampare in PDF. Un estratto dal report dimostrativo (dati reali del motore, due macchine):

> **OEE impianto: 78.5%** — disponibilità 92.8%, prestazione 86.0%, qualità 98.4% (2 macchine, 1150 pezzi buoni su 1170).
>
> | Macchina | OEE | Nota |
> |---|---|---|
> | tornio-cnc-01 | 67.8% | guasto cuscinetto 45m + setup 20m |
> | fresa-02 | **90.0%** | world-class ≥85% |
>
> **Anomalie:** tornio-cnc-01 — temperatura a 4.4σ dalla linea di base.
> **Vita utile residua:** limite 85°C raggiunto tra ~2h 10m.
> **Contromisure:** ispezionare e sostituire il cuscinetto mandrino entro 48h.

Il report completo (HTML, stampabile in PDF) è allegato a questa proposta.

### 4.6 Smistamento Nativo: Manutenzione, Logistica, Pianificazione

Con i tre modi industriali (**MAINTAIN**, **MOVE**, **PLAN**) la griglia distingue automaticamente categorie di comunicazione che prima si confondevano:

- **MAINTAIN (FABRICATE×MAINTAIN)** — "lubrificare il mandrino", "sostituire il cuscinetto usurato", "manutenzione preventiva programmata" finiscono in una cella dedicata alla manutenzione, separata dalla produzione attiva e dal fermo macchina. Lo smistamento dei ticket di manutenzione diventa immediato.
- **MOVE (FABRICATE×MOVE)** — "spostare i pallet finiti a spedizione", "movimentare il semilavorato in linea", "trasportare le travi al cantiere" vengono riconosciuti come logistica/movimentazione, non come produzione — utile per separare i colli di bottiglia di flusso materiali dai problemi di lavorazione.
- **PLAN (FABRICATE×PLAN / CONSTRUCT×PLAN)** — "pianificare il lotto", "schedulare le fasi del cantiere", "sequenziare gli ordini di lavoro" cadono in una cella di pianificazione, distinguendo le attività di programmazione dall'esecuzione.

In pratica: i flussi di manutenzione, logistica e pianificazione si auto-instradano verso le persone e i cruscotti giusti, senza regole manuali.

---

## 5. Esempi di Classificazione Reali

> *Nota: i punteggi seguenti provengono da una sessione di riferimento. Con l'introduzione dei modi industriali (MAINTAIN, MOVE, PLAN) alcune di queste frasi possono ora instradarsi su celle più specifiche (es. surriscaldamento → FABRICATE×MAINTAIN). Gli esempi vengono ri-misurati a ogni rilascio sui dati reali del cliente durante l'onboarding.*

### Esempio 1: Segnalazione di surriscaldamento CNC

**Input:** `"il tornio CNC ha superato i limiti di temperatura sul cuscinetto principale"`

| Dominio | Modo | Punteggio |
|---|---|---|
| FABRICATE | REST | **0.62** |
| HEAL | WORK | 0.59 |
| HEAL | WALK | 0.59 |

**Interpretazione:** La cella dominante è FABRICATE×REST — "Produzione in Attesa". Il sistema riconosce che l'attività principale (produzione) è interrotta per un problema tecnico. La compresenza di HEAL×WORK e HEAL×WALK indica che il problema ha anche una dimensione manutentiva.

### Esempio 2: Riunione del Comitato Direttivo

**Input:** `"riunione del comitato direttivo per approvare il budget"`

| Dominio | Modo | Punteggio |
|---|---|---|
| CONSTRUCT | WALK | **0.60** |
| — | — | — |

**Interpretazione:** CONSTRUCT×WALK — "Organizzare in Transito". Il sistema legge una riunione come un'attività organizzativa (CONSTRUCT) che procede (WALK) verso una decisione. Non è produzione diretta, ma è il movimento del processo decisionale.

### Esempio 3: Saldatura MIG difettosa

**Input:** `"MIG welder producing spatter on aluminum joint"`

| Dominio | Modo | Punteggio |
|---|---|---|
| FABRICATE | REST | 0.58 |
| FABRICATE | WORK | 0.58 |
| CONSTRUCT | WALK | 0.60 |

**Interpretazione:** La saldatura è produzione (FABRICATE), ma la presenza simultanea di REST (attesa/guasto) e WORK (lavoro) indica un'operazione che non sta procedendo correttamente. CONSTRUCT×WALK suggerisce che c'è un'azione organizzativa necessaria per risolvere il problema.

### Esempio 4: Report di Fermo Produzione

**Input:** `"Il reparto produzione segnala un fermo macchina sul centro di lavoro a 5 assi. Sensore di vibrazione fuori soglia. Necessaria manutenzione straordinaria. Impatto sulla consegna ordine 4521."`

| Dominio | Modo | Punteggio |
|---|---|---|
| FABRICATE | REST | **0.62** |

**Interpretazione:** Classificazione forte e pulita. Physis identifica immediatamente la cella FABRICATE×REST: la produzione è ferma. La presenza del riferimento all'ordine 4521 permette di collegare il fermo macchina a un impatto commerciale, attivando automaticamente le macchine Chronos (temporale) e Lex (contratti).

---

## 6. Prezzi

| Piano | Prezzo | Cosa include |
|---|---|---|
| **Prova Gratuita** | **€ 0** | 7 giorni, 50 classificazioni, accesso a tutte le ontologie |
| **Industriale** | **€ 1.500/mese** | Classificazioni illimitate, 5 utenti, web app Praxis, supporto email, rapporto di turno automatico (email/PDF), monitoraggio OEE e anomalie |
| **Enterprise** | **€ 5.000/mese** | Tutto del piano Industriale più: istanza dedicata, macchine Polis/Elenchus complete, ontologie custom, formazione del personale, SLA garantito |

### Confronto ROI

| Ruolo | Costo Annuo |
|---|---|
| Facilitatore di processo umano (part-time) | € 50.000 – 80.000/anno |
| Physis Industriale | € 18.000/anno |
| Physis Enterprise | € 60.000/anno |

Physis non sostituisce il facilitatore umano: lo **potenzia**. Un singolo facilitatore di Generalmeccanica può seguire 5× più clienti con Physis che senza.

---

## 7. Architettura del Sistema

```
Cliente (report, email, log)
       │
       ▼
  ┌──────────────┐
  │  Physis API  │─── BGE-base-en-v1.5 (embedding 768-dim)
  │  (Railway)   │─── Ontologie (630+ voci, 10 ontologie + 20 di dominio)
  └──────┬───────┘
         │
         ▼
   ┌─────────────┐
   │ Griglia     │─── 70 celle (5×14)
   │ Semiotica   │─── Punteggi di similarità
   └──────┬──────┘
          │
          ▼
   ┌─────────────┐
   │  Macchine   │─── Polis, Elenchus, Chronos, Lex, Techne
   └─────────────┘
          │
          ▼
   Report, Dashboard, Alert
```

L'API live è accessibile all'indirizzo:
**https://angelic-embrace-production-61dd.up.railway.app**

Il cruscotto web è disponibile su:
**https://praxisweb.xyz**

---

## 8. Prossimi Passi

1. **Attivazione prova gratuita** — Riceverete le credenziali API entro 24 ore. 50 classificazioni incluse per testare sui vostri dati reali, più un rapporto di turno generato sui dati OEE di una vostra macchina.
2. **Workshop di onboarding** — I nostri ingegneri semiotici lavorano con voi per configurare le ontologie sui vostri casi d'uso specifici.
3. **Primo report** — Alla fine del periodo di prova, riceverete un report personalizzato che mostra l'impatto misurato sui vostri processi.
4. **Passaggio al piano Industriale** — Attivazione immediata, nessun downtime.

### Contatti

Per attivare la prova gratuita o richiedere un incontro conoscitivo:

- **Email:** physis@praxisweb.xyz
- **API:** https://angelic-embrace-production-61dd.up.railway.app
- **Web App:** https://praxisweb.xyz

---

*Physis — Universale. Preciso. Operativo.*
