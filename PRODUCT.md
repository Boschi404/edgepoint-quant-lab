# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Primary: il proprietario del progetto (leob3), trader/ricercatore quantitativo, uso quotidiano su workstation Windows per ricercare, validare e preparare all'export strategie di trading algoritmico. Direzione confermata ma non ancora attuale: la piattaforma deve essere mostrabile e adottabile da altri trader — la UI deve essere comprensibile oltre l'autore.

## Product Purpose

EdgePoint Quant Lab è una piattaforma locale e modulare per la ricerca di strategie quantitative: ingerisce dati di mercato, li normalizza e ne valida la qualità, carica strategie come plugin Rust, genera e valuta set di parametri (search con budget), esegue backtest deterministici, calcola metriche classiche e di stabilità, produce validazioni statistiche (walk-forward, Monte Carlo, sensibilità baseline), salva risultati incrementali con checkpoint/recovery, ed espone una Web UI realtime e artifact pronti per bot Python ed EA MT5. Success = portare dati grezzi a strategia validata ed esportabile con run riproducibili, senza cambio strumento.

## Positioning

L'insieme delle tre proprietà insieme — nessun prodotto vicino le può coprire tutte e tre in verità:
1. **Determinismo + recovery**: stessa configurazione = stessi risultati; ogni run è riprendibile da checkpoint (startup recovery, checkpoint atomici per componente).
2. **Validazione statistica integrata** prima dell'export live: walk-forward fold, Monte Carlo bootstrap, stress e decay — non un add-on ma una fase della pipeline.
3. **Pipeline unica locale**: dati → search → backtest → validazione → artifact export (bot Python, MT5 EA) senza swivel-chair tra strumenti.

## Operating Context

- Strumento locale monoutente su Windows; API Rust/Axum su :8080, dashboard Vite/React su :3000 con proxy `/api`; WebSocket per progress realtime.
- I run di ricerca durano da minuti a ore: il monitoraggio nel tempo (stato, stage, percentuale, best score corrente) è parte normale dell'uso, non un caso limite.
- Configurazioni TOML (app, pipeline, datasets, logging); catalogo run in SQLite sotto `runs/`; artifact JSON colonnari compatti.
- Gli artifact alimentano consumer esterni: bot Python ed EA MetaTrader 5.
- Documentazione e README in italiano; codice e API in inglese.

## Capabilities and Constraints

- Capacità confermate: ingest CSV configurabile, normalizzazione, validazione qualità dati, registry strategie plugin statico, generazione parametri con budget deterministico, backtest con execution model (order intent/fill, vincoli), metriche classiche + stabilità, walk-forward e Monte Carlo, risultati incrementali JSONL, compaction colonnare, backup manifest, recovery endpoint, UI realtime con eventi progress tipizzati (stage/percent/message/best_score_so_far/current/total/worker_id).
- Vincoli: piattaforma infrastrutturale, NON una promessa di profittabilità — la strategia inclusa è una fixture d'integrazione, non una strategia pronta per il live. Nessun layer di autenticazione (binding localhost). Monoutente.
- Fatto undecided esplicito: nessuna decisione presa su multi-utente/remoto.

## Brand Commitments

- Nome ufficiale confermato (2026-08-26): **EdgePoint Quant Lab**. L'h1 attuale "Quant System" e il nome repo interno sono legacy da migrare, non nomi alternativi.
- Repo pubblica GitHub: Boschi404/edgepoint-quant-lab.
- Nessun asset di brand esiste ancora (logo/favicon): il redesign visivo è libero, senza asset da preservare.

## Evidence on Hand

- Suite test verificata in sessione: 17/17 pass, e2e smoke end-to-end verde, UI production build pulita.
- Fixture dati reale: `data/sample_ohlcv.csv`; bundle di debug con environment reale in `debug-bundles/`.
- Critique strutturata salvata: `.impeccable/critique/2026-08-25T22-31-22Z__ui-src-main-tsx.md` (11/40, 2 P0 robustezza, verdict category-interchangeable).
- Assenze da non fabbricare: nessun testimonial, benchmark di performance, claim di profittabilità o cliente esistente.

## Product Principles

1. **La riproducibilità prima di tutto**: stessa config = stessi risultati; ogni run è riprendibile. L'UI deve rendere visibile questo valore, non nasconderlo.
2. **Onestà statistica**: nulla passa al live senza gate di validazione; i numeri senza contesto statistico non sono risultati.
3. **Una pipeline, zero attriti**: dal dato grezzo all'artifact esportato in un unico flusso locale.
4. **L'attenzione del ricercatore è la risorsa scarsa**: durante run di ore la superficie deve raccontare cosa sta succedendo ora (progresso, best score, vita del run) senza richiedere vigilanza manuale.
5. **I fallimenti sono visibili o non esistono**: lo stallo silenzioso è il difetto peggiore possibile per chi decide su numeri.
