/* ═══════════════════════════════════════════════════════════════════════════
   CloakFund ZK-Mixer — Frontend Application Logic
   ═══════════════════════════════════════════════════════════════════════════ */

const API_BASE = "http://localhost:8080";
const BASESCAN_TX = "https://sepolia.basescan.org/tx/";
const BASESCAN_ADDR = "https://sepolia.basescan.org/address/";
const STORAGE_KEY = "cloakfund_history";

// ─── State ───
let currentPhase = "receive";
let currentStealthAddr = "";
let currentPaylinkId = "";
let backendOnline = false;

// ─── DOM Helpers ───
const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => document.querySelectorAll(sel);

// ─── Init ───
document.addEventListener("DOMContentLoaded", () => {
  initStepper();
  initForms();
  initActions();
  checkBackend();
  setInterval(checkBackend, 8000);
  renderHistory();
});

// ═══════════════════════════════════════════════════════════════════════════
//  Backend Health Check
// ═══════════════════════════════════════════════════════════════════════════

async function checkBackend() {
  try {
    const res = await fetch(`${API_BASE}/health`, { signal: AbortSignal.timeout(3000) });
    if (res.ok) {
      backendOnline = true;
      $("#backendStatus").classList.add("online");
      $("#backendStatusLabel").textContent = "Backend Online";
    } else {
      throw new Error("not ok");
    }
  } catch {
    backendOnline = false;
    $("#backendStatus").classList.remove("online");
    $("#backendStatusLabel").textContent = "Backend Offline";
  }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Phase Stepper Navigation
// ═══════════════════════════════════════════════════════════════════════════

function initStepper() {
  $$(".step-btn").forEach((btn) => {
    btn.addEventListener("click", () => {
      switchPhase(btn.dataset.phase);
    });
  });
}

function switchPhase(phase) {
  currentPhase = phase;
  
  // Update step buttons
  $$(".step-btn").forEach((btn) => {
    btn.classList.remove("active");
    if (btn.dataset.phase === phase) btn.classList.add("active");
  });

  // Update panels
  $$(".phase-panel").forEach((p) => p.classList.remove("active"));
  $(`#panel-${phase}`).classList.add("active");
}

// ═══════════════════════════════════════════════════════════════════════════
//  Forms
// ═══════════════════════════════════════════════════════════════════════════

function initForms() {
  // --- Phase 1: Receive ---
  $("#form-receive").addEventListener("submit", async (e) => {
    e.preventDefault();
    const ens = $("#inp-ens").value.trim();
    if (!ens) return;

    setBtnLoading("#btn-generate", true);
    try {
      const res = await fetch(`${API_BASE}/api/v1/paylink`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ ensName: ens }),
      });

      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.error || `HTTP ${res.status}`);
      }

      const data = await res.json();
      currentStealthAddr = data.stealthAddress || data.stealth_address;
      currentPaylinkId = data.paylinkId || data.paylink_id;
      const ephPub = data.ephemeralPubkeyHex || data.ephemeral_pubkey_hex || "";

      // Display
      $("#stealth-addr-display").textContent = currentStealthAddr;
      $("#paylink-id-display").textContent = currentPaylinkId;
      $("#ephemeral-pub-display").textContent = ephPub;
      $("#receive-result").classList.remove("hidden");

      // Save to history
      addHistoryEntry({
        id: currentPaylinkId,
        ens,
        stealthAddress: currentStealthAddr,
        ephemeralPubkey: ephPub,
        status: "created",
        createdAt: new Date().toISOString(),
      });

      // Mark step 1 as completed
      $("#step-receive").classList.add("completed");

    } catch (err) {
      alert(`Error: ${err.message}`);
    } finally {
      setBtnLoading("#btn-generate", false);
    }
  });

  // --- Phase 2: Track ---
  $("#form-track").addEventListener("submit", async (e) => {
    e.preventDefault();
    const query = $("#inp-track-addr").value.trim();
    if (!query) return;

    setBtnLoading("#btn-track", true);
    try {
      await trackDeposit(query);
    } catch (err) {
      alert(`Error: ${err.message}`);
    } finally {
      setBtnLoading("#btn-track", false);
    }
  });

  // --- Phase 3: Withdraw ---
  $("#form-withdraw").addEventListener("submit", async (e) => {
    e.preventDefault();
    const secret = $("#inp-secret").value.trim();
    const nullifier = $("#inp-nullifier").value.trim();
    const recipient = $("#inp-recipient").value.trim();

    if (!secret || !nullifier || !recipient) return;
    if (!recipient.startsWith("0x") || recipient.length !== 42) {
      alert("Please enter a valid Ethereum address (0x... 42 chars)");
      return;
    }

    setBtnLoading("#btn-withdraw", true);
    try {
      const res = await fetch(`${API_BASE}/api/v1/withdraw`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          secretHex: secret,
          nullifierHex: nullifier,
          recipientAddress: recipient,
        }),
      });

      const data = await res.json();

      const resultBox = $("#withdraw-result");
      resultBox.classList.remove("hidden", "error");

      if (res.ok && data.txHash) {
        $("#withdraw-result-title").textContent = "✅ Withdrawal Submitted!";
        $("#withdraw-result-msg").textContent = `0.0001 ETH is being sent to ${recipient}`;
        $("#withdraw-tx-display").textContent = data.txHash;
        $("#withdraw-tx-link").href = `${BASESCAN_TX}${data.txHash}`;
        $("#withdraw-tx-row").classList.remove("hidden");

        // Mark step completed
        $("#step-withdraw").classList.add("completed");

        // Update history
        updateHistoryWithdrawal(secret, data.txHash, recipient);
      } else {
        resultBox.classList.add("error");
        $("#withdraw-result-title").textContent = "❌ Withdrawal Failed";
        $("#withdraw-result-msg").textContent = data.error || "Unknown error";
        $("#withdraw-tx-row").classList.add("hidden");
      }
    } catch (err) {
      alert(`Network error: ${err.message}`);
    } finally {
      setBtnLoading("#btn-withdraw", false);
    }
  });
}

// ═══════════════════════════════════════════════════════════════════════════
//  Track Deposit Pipeline
// ═══════════════════════════════════════════════════════════════════════════

async function trackDeposit(query) {
  const trackResult = $("#track-result");
  trackResult.classList.remove("hidden");

  // Reset timeline
  $$(".tl-step").forEach((s) => s.classList.remove("done", "active", "failed"));
  $$(".tl-step p").forEach((p) => (p.textContent = "—"));
  $("#btn-goto-withdraw").classList.add("hidden");

  // 1. Check deposit by tx hash
  let depositData = null;

  try {
    const res = await fetch(`${API_BASE}/api/v1/deposit/status?txHash=${encodeURIComponent(query)}`, {
      signal: AbortSignal.timeout(5000),
    });
    if (res.ok) {
      depositData = await res.json();
    }
  } catch {
    // Try alternative endpoint or method — fallback
  }

  if (!depositData) {
    // If no deposit found, check by looking at paylink stealth addrs
    // For now show a pending state
    setTimelineStep("tl-deposit", "active", "Waiting for deposit...");
    startAutoRefreshTrack(query);
    return;
  }

  // Populate timeline from deposit data
  const deposit = depositData.deposit || depositData;

  if (deposit.txHash || deposit.tx_hash) {
    const txH = deposit.txHash || deposit.tx_hash;
    setTimelineStep("tl-deposit", "done", `Tx: ${txH.slice(0, 18)}…`);
  }

  const confStatus = deposit.confirmationStatus || deposit.confirmation_status;
  if (confStatus === "finalized" || confStatus === "confirmed") {
    const confs = deposit.confirmations || "✓";
    setTimelineStep("tl-confirmed", "done", `${confs} confirmations — ${confStatus}`);
  } else if (confStatus) {
    setTimelineStep("tl-confirmed", "active", `${confStatus} (${deposit.confirmations || 0} confs)`);
  }

  // Check sweep status
  const sweepStatus = deposit.sweepStatus || deposit.sweep_status;
  if (sweepStatus === "completed") {
    const sweepTx = deposit.sweepTxHash || deposit.sweep_tx_hash || "";
    setTimelineStep("tl-swept", "done", sweepTx ? `Tx: ${sweepTx.slice(0, 18)}…` : "Deposited to pool");
  } else if (sweepStatus === "in_progress") {
    setTimelineStep("tl-swept", "active", "Sweeping to Privacy Pool...");
  } else if (sweepStatus === "failed") {
    setTimelineStep("tl-swept", "failed", "Sweep failed — will retry");
  }

  // Check privacy note
  if (deposit.note || deposit.privacyNote) {
    const note = deposit.note || deposit.privacyNote;
    setTimelineStep("tl-note", "done", `Commitment: ${(note.commitmentHex || note.commitment_hex || "").slice(0, 20)}…`);
    
    // Mark step 2 completed
    $("#step-track").classList.add("completed");

    // Show withdraw button and pre-fill
    $("#btn-goto-withdraw").classList.remove("hidden");
    if (note.secretHex || note.secret_hex) {
      $("#inp-secret").value = note.secretHex || note.secret_hex;
    }
    if (note.nullifierHex || note.nullifier_hex) {
      $("#inp-nullifier").value = note.nullifierHex || note.nullifier_hex;
    }
  } else if (sweepStatus === "completed") {
    setTimelineStep("tl-note", "active", "Generating privacy note...");
  }

  // Check withdrawal
  if (deposit.withdrawnTxHash || deposit.withdrawn_tx_hash) {
    const wTx = deposit.withdrawnTxHash || deposit.withdrawn_tx_hash;
    setTimelineStep("tl-withdrawn", "done", `Tx: ${wTx.slice(0, 18)}…`);
  }

  // Update history
  const histEntry = getHistoryByAddr(deposit.toAddress || deposit.to_address);
  if (histEntry) {
    histEntry.status = sweepStatus === "completed" ? "swept" : "funded";
    saveHistory();
    renderHistory();
  }
}

function setTimelineStep(stepId, state, detail) {
  const step = $(`#${stepId}`);
  step.classList.remove("done", "active", "failed");
  step.classList.add(state);
  step.querySelector("p").textContent = detail;
}

let autoRefreshInterval = null;

function startAutoRefreshTrack(query) {
  if (autoRefreshInterval) clearInterval(autoRefreshInterval);
  autoRefreshInterval = setInterval(async () => {
    try {
      await trackDeposit(query);
    } catch { /* silently retry */ }
  }, 10000);
}

// ═══════════════════════════════════════════════════════════════════════════
//  Actions & Navigation
// ═══════════════════════════════════════════════════════════════════════════

function initActions() {
  // Copy stealth address
  $("#btn-copy-addr").addEventListener("click", () => {
    navigator.clipboard.writeText(currentStealthAddr).then(() => {
      const btn = $("#btn-copy-addr");
      btn.textContent = "✅";
      btn.classList.add("copied");
      setTimeout(() => {
        btn.textContent = "📋";
        btn.classList.remove("copied");
      }, 1500);
    });
  });

  // Go to track
  $("#btn-goto-track").addEventListener("click", () => {
    if (currentStealthAddr) {
      // Pre-fill with recent tx hash or stealth addr
      const histEntries = getHistory();
      const latest = histEntries.find((h) => h.stealthAddress === currentStealthAddr);
      if (latest && latest.fundingTxHash) {
        $("#inp-track-addr").value = latest.fundingTxHash;
      } else {
        $("#inp-track-addr").value = currentStealthAddr;
      }
    }
    switchPhase("track");
  });

  // Go to withdraw
  $("#btn-goto-withdraw").addEventListener("click", () => {
    switchPhase("withdraw");
  });

  // Clear history
  $("#btn-clear-history").addEventListener("click", () => {
    if (confirm("Clear all local transaction history?")) {
      localStorage.removeItem(STORAGE_KEY);
      renderHistory();
    }
  });
}

// ═══════════════════════════════════════════════════════════════════════════
//  Local History (localStorage)
// ═══════════════════════════════════════════════════════════════════════════

function getHistory() {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) || "[]");
  } catch {
    return [];
  }
}

function saveHistory(entries) {
  if (entries) {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(entries));
  } else {
    // Get and re-save current
    const h = getHistory();
    localStorage.setItem(STORAGE_KEY, JSON.stringify(h));
  }
}

function addHistoryEntry(entry) {
  const history = getHistory();
  // Avoid duplicates
  if (history.find((h) => h.id === entry.id)) return;
  history.unshift(entry);
  saveHistory(history);
  renderHistory();
}

function getHistoryByAddr(addr) {
  if (!addr) return null;
  const history = getHistory();
  return history.find((h) => h.stealthAddress && h.stealthAddress.toLowerCase() === addr.toLowerCase());
}

function updateHistoryWithdrawal(secret, txHash, recipient) {
  const history = getHistory();
  // Find by secret match (best-effort)
  for (const entry of history) {
    if (entry.status !== "withdrawn") {
      entry.status = "withdrawn";
      entry.withdrawTxHash = txHash;
      entry.withdrawRecipient = recipient;
      break;
    }
  }
  saveHistory(history);
  renderHistory();
}

function renderHistory() {
  const list = $("#history-list");
  const empty = $("#history-empty");
  const clearBtn = $("#btn-clear-history");
  const history = getHistory();

  if (history.length === 0) {
    empty.classList.remove("hidden");
    list.innerHTML = "";
    clearBtn.classList.add("hidden");
    return;
  }

  empty.classList.add("hidden");
  clearBtn.classList.remove("hidden");

  list.innerHTML = history
    .map((h) => {
      const statusClass = h.status || "created";
      const statusLabel = statusClass.charAt(0).toUpperCase() + statusClass.slice(1);
      const addrDisplay = h.stealthAddress
        ? `${h.stealthAddress.slice(0, 10)}…${h.stealthAddress.slice(-8)}`
        : "—";
      const timeDisplay = h.createdAt
        ? new Date(h.createdAt).toLocaleString()
        : "";

      return `
        <div class="history-item" data-addr="${h.stealthAddress || ""}" onclick="onHistoryClick(this)">
          <div class="hi-header">
            <span class="hi-ens">${escapeHtml(h.ens || "Unknown")}</span>
            <span class="hi-status ${statusClass}">${statusLabel}</span>
          </div>
          <div class="hi-addr">${addrDisplay}</div>
          <div class="hi-time">${timeDisplay}</div>
        </div>
      `;
    })
    .join("");
}

function onHistoryClick(el) {
  const addr = el.dataset.addr;
  if (addr) {
    $("#inp-track-addr").value = addr;
    switchPhase("track");
  }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Utilities
// ═══════════════════════════════════════════════════════════════════════════

function setBtnLoading(selector, loading) {
  const btn = $(selector);
  const text = btn.querySelector(".btn-text");
  const loader = btn.querySelector(".btn-loader");

  if (loading) {
    btn.disabled = true;
    if (text) text.classList.add("hidden");
    if (loader) loader.classList.remove("hidden");
  } else {
    btn.disabled = false;
    if (text) text.classList.remove("hidden");
    if (loader) loader.classList.add("hidden");
  }
}

function escapeHtml(str) {
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}
