"use client"

import * as React from "react"
import {
  Bar,
  BarChart,
  CartesianGrid,
  Line,
  LineChart,
  XAxis,
  YAxis,
} from "recharts"

import { apiGet } from "@/lib/api"
import type { AdminStatsResponse, AdminStatsSnapshot } from "@/lib/types"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/components/ui/chart"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Skeleton } from "@/components/ui/skeleton"

type MetricKey =
  | "user_count"
  | "bucket_count"
  | "image_link_count"
  | "unique_file_count"
  | "send_count"
  | "b2_object_count"
  | "b2_bytes"

type RangeKey = "7" | "30" | "90" | "all"

type ChartRow = {
  date: string
  value: number | null
}

type SummaryMetric = {
  key: MetricKey
  label: string
  description: string
  format?: (value: number | null) => string
}

const RANGE_OPTIONS: Array<{ value: RangeKey; label: string }> = [
  { value: "7", label: "7 days" },
  { value: "30", label: "30 days" },
  { value: "90", label: "90 days" },
  { value: "all", label: "All time" },
]

const METRIC_OPTIONS: Array<{ value: MetricKey; label: string; chartType: "line" | "bar" }> = [
  { value: "user_count", label: "Users", chartType: "line" },
  { value: "bucket_count", label: "Buckets", chartType: "line" },
  { value: "image_link_count", label: "Image links", chartType: "line" },
  { value: "unique_file_count", label: "Unique files", chartType: "line" },
  { value: "send_count", label: "Sends", chartType: "bar" },
  { value: "b2_object_count", label: "B2 objects", chartType: "line" },
  { value: "b2_bytes", label: "B2 storage", chartType: "line" },
]

const SUMMARY_METRICS: SummaryMetric[] = [
  { key: "user_count", label: "Users", description: "Accounts with access" },
  { key: "bucket_count", label: "Buckets", description: "Total buckets" },
  { key: "image_link_count", label: "Image links", description: "Saved image links" },
  { key: "unique_file_count", label: "Unique files", description: "Stored source files" },
  { key: "send_count", label: "Sends", description: "Total sends recorded" },
  { key: "b2_object_count", label: "B2 objects", description: "Tracked storage objects" },
  {
    key: "b2_bytes",
    label: "B2 storage",
    description: "Tracked B2 bytes",
    format: formatBytes,
  },
]

const CHART_CONFIG = {
  value: {
    label: "Value",
    color: "var(--chart-1, hsl(221 83% 53%))",
  },
} satisfies ChartConfig

function formatNumber(value: number | null) {
  if (value == null) {
    return "—"
  }

  return value.toLocaleString()
}

function formatBytes(value: number | null) {
  if (value == null) {
    return "—"
  }

  if (value < 1024) {
    return `${value} B`
  }

  const units = ["KiB", "MiB", "GiB", "TiB"]
  let scaled = value
  let unitIndex = -1

  while (scaled >= 1024 && unitIndex < units.length - 1) {
    scaled /= 1024
    unitIndex += 1
  }

  return `${scaled.toFixed(1)} ${units[unitIndex]}`
}

function snapshotValue(snapshot: AdminStatsSnapshot, metric: MetricKey) {
  return snapshot[metric]
}

function isoDateOffset(baseDate: string, offsetDays: number) {
  const date = new Date(`${baseDate}T00:00:00.000Z`)
  date.setUTCDate(date.getUTCDate() + offsetDays)
  return date.toISOString().slice(0, 10)
}

function buildChartRows(history: AdminStatsSnapshot[], metric: MetricKey) {
  return history.map((snapshot, index) => {
    if (metric === "send_count") {
      const previous = history[index - 1]
      return {
        date: snapshot.snapshot_date,
        value: previous ? snapshot.send_count - previous.send_count : snapshot.send_count,
      }
    }

    return {
      date: snapshot.snapshot_date,
      value: snapshotValue(snapshot, metric),
    }
  })
}

function filterRows(rows: ChartRow[], endDate: string, range: RangeKey) {
  if (range === "all") {
    return rows
  }

  const days = Number(range)
  const cutoff = isoDateOffset(endDate, -(days - 1))
  return rows.filter((row) => row.date >= cutoff)
}

function storageMessage(stats: AdminStatsResponse) {
  if (stats.storage.available) {
    return null
  }

  if (!stats.storage.configured) {
    return "Storage is not configured for this environment."
  }

  return "Storage metrics are not available for the latest snapshot yet."
}

export function AdminStatsDashboard() {
  const [stats, setStats] = React.useState<AdminStatsResponse | null>(null)
  const [loading, setLoading] = React.useState(true)
  const [error, setError] = React.useState<string | null>(null)
  const [metric, setMetric] = React.useState<MetricKey>("user_count")
  const [range, setRange] = React.useState<RangeKey>("30")

  React.useEffect(() => {
    let active = true

    async function loadStats() {
      try {
        const response = await apiGet<AdminStatsResponse>("/api/admin/stats")
        if (!active) {
          return
        }

        setStats(response)
        setError(null)
      } catch {
        if (!active) {
          return
        }

        setError("Could not load admin stats. Try again.")
      } finally {
        if (active) {
          setLoading(false)
        }
      }
    }

    loadStats()

    return () => {
      active = false
    }
  }, [])

  const selectedMetric = METRIC_OPTIONS.find((option) => option.value === metric) ?? METRIC_OPTIONS[0]
  const chartRows = React.useMemo(() => {
    if (!stats || stats.history.length === 0) {
      return []
    }

    return filterRows(buildChartRows(stats.history, metric), stats.current.snapshot_date, range)
  }, [metric, range, stats])

  if (loading) {
    return (
      <div className="space-y-6">
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          {Array.from({ length: 7 }).map((_, index) => (
            <Card key={index}>
              <CardHeader>
                <Skeleton className="h-4 w-24" />
                <Skeleton className="h-3 w-32" />
              </CardHeader>
              <CardContent>
                <Skeleton className="h-8 w-28" />
              </CardContent>
            </Card>
          ))}
        </div>
        <Card>
          <CardHeader className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
            <div className="space-y-2">
              <Skeleton className="h-5 w-36" />
              <Skeleton className="h-4 w-48" />
            </div>
            <div className="flex gap-2">
              <Skeleton className="h-9 w-28" />
              <Skeleton className="h-9 w-28" />
            </div>
          </CardHeader>
          <CardContent>
            <Skeleton className="h-72 w-full" />
          </CardContent>
        </Card>
      </div>
    )
  }

  if (error || !stats) {
    return (
      <Alert>
        <AlertTitle>Admin stats unavailable</AlertTitle>
        <AlertDescription>{error ?? "Could not load admin stats. Try again."}</AlertDescription>
      </Alert>
    )
  }

  const storageNotice = storageMessage(stats)

  return (
    <div className="space-y-6">
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {SUMMARY_METRICS.map((summaryMetric) => {
          const value = snapshotValue(stats.current, summaryMetric.key)
          const formatter = summaryMetric.format ?? formatNumber

          return (
            <Card key={summaryMetric.key}>
              <CardHeader>
                <CardTitle>{summaryMetric.label}</CardTitle>
                <CardDescription>{summaryMetric.description}</CardDescription>
              </CardHeader>
              <CardContent>
                <p className="text-3xl font-semibold tracking-tight">{formatter(value)}</p>
              </CardContent>
            </Card>
          )
        })}
      </div>

      {storageNotice ? (
        <Alert>
          <AlertTitle>{storageNotice}</AlertTitle>
          <AlertDescription>
            {stats.storage.first_complete_history_date
              ? `Complete storage history begins on ${stats.storage.first_complete_history_date}.`
              : "Complete storage history is not available yet."}
          </AlertDescription>
        </Alert>
      ) : null}

      <Card>
        <CardHeader className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
          <div>
            <CardTitle>{selectedMetric.label} over time</CardTitle>
            <CardDescription>
              Historical aggregate snapshots through {stats.current.snapshot_date}.
            </CardDescription>
          </div>
          <div className="flex flex-col gap-2 sm:flex-row">
            <Select value={metric} onValueChange={(value) => setMetric(value as MetricKey)}>
              <SelectTrigger aria-label="Metric">
                <SelectValue placeholder="Metric" />
              </SelectTrigger>
              <SelectContent>
                {METRIC_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>

            <Select value={range} onValueChange={(value) => setRange(value as RangeKey)}>
              <SelectTrigger aria-label="Range">
                <SelectValue placeholder="Range" />
              </SelectTrigger>
              <SelectContent>
                {RANGE_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </CardHeader>
        <CardContent>
          {chartRows.length === 0 ? (
            <Alert>
              <AlertTitle>No historical snapshots yet.</AlertTitle>
              <AlertDescription>
                The dashboard can show trend charts once snapshot history is available.
              </AlertDescription>
            </Alert>
          ) : (
            <ChartContainer className="h-72 w-full" config={CHART_CONFIG}>
              {selectedMetric.chartType === "bar" ? (
                <BarChart data={chartRows}>
                  <CartesianGrid vertical={false} />
                  <XAxis dataKey="date" tickLine={false} axisLine={false} minTickGap={24} />
                  <YAxis tickLine={false} axisLine={false} width={48} />
                  <ChartTooltip content={<ChartTooltipContent />} />
                  <Bar dataKey="value" fill="var(--color-value)" radius={[8, 8, 0, 0]} />
                </BarChart>
              ) : (
                <LineChart data={chartRows}>
                  <CartesianGrid vertical={false} />
                  <XAxis dataKey="date" tickLine={false} axisLine={false} minTickGap={24} />
                  <YAxis tickLine={false} axisLine={false} width={48} />
                  <ChartTooltip content={<ChartTooltipContent />} />
                  <Line
                    type="monotone"
                    dataKey="value"
                    stroke="var(--color-value)"
                    strokeWidth={2}
                    dot={false}
                  />
                </LineChart>
              )}
            </ChartContainer>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
