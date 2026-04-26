<script setup lang="ts">
import { ref } from 'vue'

type Status = 'idle' | 'solving' | 'done'

interface SolveResult {
  pass_pct: number
  bet_pct: number
}

const CARDS = ['J', 'Q', 'K'] as const

const SITUATIONS = [
  { label: 'P1 · first to act', suffix: '' },
  { label: 'P2 · vs check',     suffix: 'p' },
  { label: 'P2 · vs bet',       suffix: 'b' },
  { label: 'P1 · vs check-raise', suffix: 'pb' },
] as const

const status     = ref<Status>('idle')
const iterations = ref(10000)
const card       = ref<string>('K')
const situation  = ref<string>('')        // store the suffix string directly
const progress   = ref({ pct: 0, exploitability: 0 })
const result     = ref<SolveResult | null>(null)
const submitted  = ref({ card: 'K', situation: '' })

async function calculate() {
  status.value = 'solving'
  result.value = null
  progress.value = { pct: 0, exploitability: 0 }
  submitted.value = { card: card.value, situation: situation.value }

  const { job_id } = await fetch('/solve', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ card: card.value, situation: situation.value, iterations: iterations.value }),
  }).then(r => r.json())

  await new Promise<void>((resolve, reject) => {
    const ws = new WebSocket(`/ws/${job_id}`)
    ws.onmessage = (e) => {
      const msg = JSON.parse(e.data)
      if (msg.type === 'progress') {
        progress.value = {
          pct: Math.round((msg.iteration / msg.total) * 100),
          exploitability: msg.exploitability,
        }
      } else {
        ws.close()
        resolve()
      }
    }
    ws.onerror = reject
  })

  result.value = await fetch(`/result/${job_id}`).then(r => r.json())
  status.value = 'done'
}
</script>

<template>
  <header>
    <h1>Kuhn Poker Solver</h1>
  </header>

  <main>
    <div class="field">
      <span class="field-label">Your card</span>
      <div class="btn-group">
        <button
          v-for="c in CARDS"
          :key="c"
          :class="{ active: card === c }"
          :disabled="status === 'solving'"
          @click="card = c"
        >{{ c }}</button>
      </div>
    </div>

    <div class="field">
      <span class="field-label">Situation</span>
      <div class="btn-group">
        <button
          v-for="s in SITUATIONS"
          :key="s.suffix"
          :class="{ active: situation === s.suffix }"
          :disabled="status === 'solving'"
          @click="situation = s.suffix"
        >{{ s.label }}</button>
      </div>
    </div>

    <div class="field row">
      <label class="field-label" for="iters">Iterations</label>
      <input
        id="iters"
        v-model.number="iterations"
        type="number"
        min="1000"
        max="100000"
        step="1000"
        :disabled="status === 'solving'"
      />
      <button class="primary" :disabled="status === 'solving'" @click="calculate">
        {{ status === 'solving' ? 'Solving…' : status === 'done' ? 'Re-calculate' : 'Calculate' }}
      </button>
    </div>

    <div v-if="status === 'solving'" class="progress-wrap">
      <div class="progress-bar" :style="{ width: progress.pct + '%' }"></div>
      <span class="progress-label">{{ progress.pct }}% — exploitability {{ progress.exploitability.toFixed(4) }}</span>
    </div>

    <template v-if="status === 'done' && result">
      <div class="result-card">
        <p class="spot-title">{{ submitted.card }} · {{ SITUATIONS.find(s => s.suffix === submitted.situation)?.label }}</p>
        <div class="action-row">
          <div class="action">
            <span class="action-label pass">Pass</span>
            <span class="action-pct pass">{{ (result.pass_pct * 100).toFixed(1) }}%</span>
          </div>
          <div class="action-bar-wrap">
            <div class="action-fill pass-fill" :style="{ width: (result.pass_pct * 100) + '%' }"></div>
            <div class="action-fill bet-fill"  :style="{ width: (result.bet_pct  * 100) + '%' }"></div>
          </div>
          <div class="action">
            <span class="action-label bet">Bet</span>
            <span class="action-pct bet">{{ (result.bet_pct * 100).toFixed(1) }}%</span>
          </div>
        </div>
      </div>
    </template>
  </main>
</template>
