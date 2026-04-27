<script setup lang="ts">
import { ref, computed } from 'vue'

type Status = 'idle' | 'solving' | 'done'

interface CardResult {
  pass_pct: number
  bet_pct: number
}

interface SolveResult {
  j: CardResult
  q: CardResult
  k: CardResult
}

interface GameColumn {
  player: string
  subtitle: string
  situation: string
}

const CARDS = ['J', 'Q', 'K'] as const

const SITUATIONS = [
  { label: 'P1 · first to act',    suffix: '' },
  { label: 'P2 · vs check',        suffix: 'p' },
  { label: 'P2 · vs bet',          suffix: 'b' },
  { label: 'P1 · vs check-raise',  suffix: 'pb' },
] as const

const status     = ref<Status>('idle')
const iterations = ref(10000)
const path            = ref<string[]>([])
const activeColIndex  = ref(0)
const progress        = ref({ pct: 0, exploitability: 0 })
const result     = ref<SolveResult | null>(null)
const submittedSituation = ref('')

const columns = computed((): GameColumn[] => {
  const cols: GameColumn[] = [
    { player: 'P1', subtitle: 'first to act', situation: '' },
  ]
  if (!path.value[0]) return cols

  if (path.value[0] === 'p') {
    cols.push({ player: 'P2', subtitle: 'vs check', situation: 'p' })
    if (path.value[1] === 'b') {
      cols.push({ player: 'P1', subtitle: 'vs check-raise', situation: 'pb' })
    }
  } else {
    cols.push({ player: 'P2', subtitle: 'vs bet', situation: 'b' })
  }
  return cols
})

const situation = computed(() => columns.value[activeColIndex.value]?.situation ?? '')

function selectAction(colIndex: number, action: string) {
  path.value = [...path.value.slice(0, colIndex), action]
  activeColIndex.value = columns.value.length - 1
}

async function calculate() {
  status.value = 'solving'
  result.value = null
  progress.value = { pct: 0, exploitability: 0 }
  submittedSituation.value = situation.value

  const { job_id } = await fetch('/solve', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ situation: situation.value, iterations: iterations.value }),
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

function cardResult(card: typeof CARDS[number]): CardResult {
  if (!result.value) return { pass_pct: 0, bet_pct: 0 }
  return result.value[card.toLowerCase() as 'j' | 'q' | 'k']
}
</script>

<template>
  <header>
    <h1>Kuhn Poker Solver</h1>
  </header>

  <main>
    <div class="field">
      <span class="field-label">Situation</span>
      <div class="situation-columns">
        <div
          v-for="(col, i) in columns"
          :key="col.situation"
          class="situation-col"
          :class="{ active: i === activeColIndex }"
        >
          <div class="col-header" @click="activeColIndex = i">
            <span class="col-player">{{ col.player }}</span>
            <span class="col-subtitle">{{ col.subtitle }}</span>
          </div>
          <button
            v-for="action in [{ label: 'Pass', key: 'p' }, { label: 'Bet', key: 'b' }]"
            :key="action.key"
            class="col-action"
            :class="{ selected: path[i] === action.key }"
            :disabled="status === 'solving'"
            @click="selectAction(i, action.key)"
          >{{ action.label }}</button>
        </div>
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
        {{ status === 'solving' ? 'Solving…' : 'Calculate' }}
      </button>
    </div>

    <div v-if="status === 'solving'" class="progress-wrap">
      <div class="progress-bar" :style="{ width: progress.pct + '%' }"></div>
      <span class="progress-label">{{ progress.pct }}% — exploitability {{ progress.exploitability.toFixed(4) }}</span>
    </div>

    <template v-if="status === 'done' && result">
      <div
        v-for="c in CARDS"
        :key="c"
        class="result-card"
      >
        <p class="spot-title">{{ c }} · {{ SITUATIONS.find(s => s.suffix === submittedSituation)?.label }}</p>
        <div class="action-row">
          <div class="action">
            <span class="action-label pass">Pass</span>
            <span class="action-pct pass">{{ (cardResult(c).pass_pct * 100).toFixed(1) }}%</span>
          </div>
          <div class="action-bar-wrap">
            <div class="action-fill pass-fill" :style="{ width: (cardResult(c).pass_pct * 100) + '%' }"></div>
            <div class="action-fill bet-fill"  :style="{ width: (cardResult(c).bet_pct  * 100) + '%' }"></div>
          </div>
          <div class="action">
            <span class="action-label bet">Bet</span>
            <span class="action-pct bet">{{ (cardResult(c).bet_pct * 100).toFixed(1) }}%</span>
          </div>
        </div>
      </div>
    </template>
  </main>
</template>
