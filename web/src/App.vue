<script setup lang="ts">
import { ref } from 'vue'

type Status = 'idle' | 'solving' | 'done'

interface SolveResult {
  strategy: Record<string, number[]>
  ev: number
  iterations: number
}

interface ProgressMsg {
  type: 'progress'
  iteration: number
  total: number
  exploitability: number
}

const CARDS = ['J', 'Q', 'K']

const status = ref<Status>('idle')
const iterations = ref(10000)
const progress = ref({ pct: 0, exploitability: 0 })
const result = ref<SolveResult | null>(null)

function pct(key: string, idx: number): string {
  const probs = result.value?.strategy[key]
  return probs ? (probs[idx] * 100).toFixed(1) + '%' : '—'
}

async function solve() {
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
      const msg = JSON.parse(e.data) as ProgressMsg | { type: 'done' }
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
    <div class="controls">
      <label>
        Iterations
        <input v-model.number="iterations" type="number" min="1000" max="100000" step="1000" :disabled="status === 'solving'" />
      </label>
      <button @click="solve" :disabled="status === 'solving'">
        {{ status === 'solving' ? 'Solving…' : status === 'done' ? 'Re-solve' : 'Solve' }}
      </button>
    </div>

    <div v-if="status === 'solving'" class="progress-wrap">
      <div class="progress-bar" :style="{ width: progress.pct + '%' }"></div>
      <span class="progress-label">{{ progress.pct }}% — exploitability: {{ progress.exploitability.toFixed(4) }}</span>
    </div>

    <template v-if="result">
      <p class="ev">P1 EV: <strong>{{ result.ev.toFixed(4) }}</strong> <span class="muted">(Nash = −0.0556)</span></p>

      <div class="tables">
        <section>
          <h2>Player 1 · first to act</h2>
          <table>
            <thead><tr><th>Card</th><th>Pass%</th><th>Bet%</th></tr></thead>
            <tbody>
              <tr v-for="c in CARDS" :key="c">
                <td class="card">{{ c }}</td>
                <td class="pass">{{ pct(c, 0) }}</td>
                <td class="bet">{{ pct(c, 1) }}</td>
              </tr>
            </tbody>
          </table>
        </section>

        <section>
          <h2>Player 2 · vs pass</h2>
          <table>
            <thead><tr><th>Card</th><th>Pass%</th><th>Bet%</th></tr></thead>
            <tbody>
              <tr v-for="c in CARDS" :key="c">
                <td class="card">{{ c }}</td>
                <td class="pass">{{ pct(c + 'p', 0) }}</td>
                <td class="bet">{{ pct(c + 'p', 1) }}</td>
              </tr>
            </tbody>
          </table>
        </section>

        <section>
          <h2>Player 2 · vs bet</h2>
          <table>
            <thead><tr><th>Card</th><th>Pass%</th><th>Bet%</th></tr></thead>
            <tbody>
              <tr v-for="c in CARDS" :key="c">
                <td class="card">{{ c }}</td>
                <td class="pass">{{ pct(c + 'b', 0) }}</td>
                <td class="bet">{{ pct(c + 'b', 1) }}</td>
              </tr>
            </tbody>
          </table>
        </section>

        <section>
          <h2>Player 1 · vs check-raise</h2>
          <table>
            <thead><tr><th>Card</th><th>Pass%</th><th>Bet%</th></tr></thead>
            <tbody>
              <tr v-for="c in CARDS" :key="c">
                <td class="card">{{ c }}</td>
                <td class="pass">{{ pct(c + 'pb', 0) }}</td>
                <td class="bet">{{ pct(c + 'pb', 1) }}</td>
              </tr>
            </tbody>
          </table>
        </section>
      </div>
    </template>
  </main>
</template>
