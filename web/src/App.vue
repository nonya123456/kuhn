<script setup lang="ts">
import { ref, computed } from 'vue'

type Status = 'idle' | 'solving' | 'done'

interface SolveResult {
  strategy: Record<string, number[]>
  ev: number
  iterations: number
}

const CARDS = ['J', 'Q', 'K'] as const
type Card = typeof CARDS[number]

const SITUATIONS = [
  { label: 'P1 · first to act', suffix: '' },
  { label: 'P2 · vs check',     suffix: 'p' },
  { label: 'P2 · vs bet',       suffix: 'b' },
  { label: 'P1 · vs check-raise', suffix: 'pb' },
] as const
type Situation = typeof SITUATIONS[number]

const status      = ref<Status>('idle')
const iterations  = ref(10000)
const card        = ref<Card>('K')
const situation   = ref<Situation>(SITUATIONS[0])
const progress    = ref({ pct: 0, exploitability: 0 })
const result      = ref<SolveResult | null>(null)

const infosetKey = computed(() => card.value + situation.value.suffix)

const spotProbs = computed(() => {
  if (!result.value) return null
  return result.value.strategy[infosetKey.value] ?? null
})

async function calculate() {
  status.value = 'solving'
  result.value = null
  progress.value = { pct: 0, exploitability: 0 }

  const { job_id } = await fetch('/solve', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ iterations: iterations.value }),
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
    <p class="subtitle">GTO via CFR+ · ante 1</p>
  </header>

  <main>
    <div class="field">
      <span class="field-label">Your card</span>
      <div class="btn-group">
        <button
          v-for="c in CARDS"
          :key="c"
          :class="{ active: card === c }"
          @click="card = c"
          :disabled="status === 'solving'"
        >{{ c }}</button>
      </div>
    </div>

    <div class="field">
      <span class="field-label">Situation</span>
      <div class="btn-group">
        <button
          v-for="s in SITUATIONS"
          :key="s.suffix"
          :class="{ active: situation === s }"
          @click="situation = s"
          :disabled="status === 'solving'"
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
      <button class="primary" @click="calculate" :disabled="status === 'solving'">
        {{ status === 'solving' ? 'Solving…' : status === 'done' ? 'Re-calculate' : 'Calculate' }}
      </button>
    </div>

    <div v-if="status === 'solving'" class="progress-wrap">
      <div class="progress-bar" :style="{ width: progress.pct + '%' }"></div>
      <span class="progress-label">{{ progress.pct }}% — exploitability {{ progress.exploitability.toFixed(4) }}</span>
    </div>

    <template v-if="status === 'done' && spotProbs">
      <div class="result-card">
        <p class="spot-title">{{ card }} · {{ situation.label }}</p>
        <div class="action-row">
          <div class="action">
            <span class="action-label pass">Pass</span>
            <span class="action-pct pass">{{ (spotProbs[0] * 100).toFixed(1) }}%</span>
          </div>
          <div class="action-bar-wrap">
            <div class="action-fill pass-fill" :style="{ width: (spotProbs[0] * 100) + '%' }"></div>
            <div class="action-fill bet-fill"  :style="{ width: (spotProbs[1] * 100) + '%' }"></div>
          </div>
          <div class="action">
            <span class="action-label bet">Bet</span>
            <span class="action-pct bet">{{ (spotProbs[1] * 100).toFixed(1) }}%</span>
          </div>
        </div>
      </div>
      <p class="ev">P1 EV <strong>{{ result!.ev.toFixed(4) }}</strong> <span class="muted">(Nash = −0.0556)</span></p>
    </template>
  </main>
</template>
