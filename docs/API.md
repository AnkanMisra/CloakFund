# API

POST /paylink

Generates stealth payment address.

---

GET /deposit-status?paylinkId=<id>

Returns payment confirmation, detected deposits, and aggregated balances. (Served via Convex HTTP action)

---

GET /health

Returns Convex backend health status and timestamp. (Served via Convex HTTP action)

---

POST /consolidate

Triggers treasury consolidation.

---

GET /receipts

Returns encrypted receipt list.
