import { useTranslation } from 'react-i18next'
import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { invoke } from '@tauri-apps/api/core'
import {
  Box,
  Card,
  CardContent,
  FormControl,
  Grid,
  InputLabel,
  MenuItem,
  Select,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
  Paper,
  Skeleton,
  ToggleButton,
  ToggleButtonGroup,
  TableSortLabel,
} from '@mui/material'
import DataUsageRoundedIcon from '@mui/icons-material/DataUsageRounded'
import PublicRoundedIcon from '@mui/icons-material/PublicRounded'
import AppsRoundedIcon from '@mui/icons-material/AppsRounded'

import { BasePage } from '@/components/base'
import parseTraffic from '@/utils/parse-traffic'

// 格式化流量显示
function formatTraffic(bytes: number): string {
  const [value, unit] = parseTraffic(bytes)
  return `${value} ${unit}`
}

// 排序类型
type SortBy = 'download' | 'upload' | 'total' | 'connections'

interface TrafficAggregate {
  key: string
  total_upload: number
  total_download: number
  connection_count: number
  last_active: string
}

interface TrafficSummary {
  total_upload: number
  total_download: number
  total_connections: number
  unique_domains: number
  unique_processes: number
}

// 获取时间范围起始时间
function getStartTime(range: string): string | null {
  const now = new Date()
  switch (range) {
    case 'today':
      return new Date(now.getFullYear(), now.getMonth(), now.getDate()).toISOString()
    case 'week':
      return new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000).toISOString()
    case 'month':
      return new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000).toISOString()
    case 'all':
      return null
    default:
      return null
  }
}

export default function TrafficStatsPage() {
  const { t } = useTranslation()
  const [groupBy, setGroupBy] = useState<'domain' | 'process'>('domain')
  const [timeRange, setTimeRange] = useState<'today' | 'week' | 'month' | 'all'>('today')
  const [sortBy, setSortBy] = useState<SortBy>('download')

  const startTime = useMemo(() => getStartTime(timeRange), [timeRange])
  const endTime = useMemo(() => new Date().toISOString(), [])

  // 查询聚合数据
  const { data: aggregateData, isLoading: isAggregateLoading } = useQuery({
    queryKey: ['traffic-stats', groupBy, timeRange],
    queryFn: async () => {
      const result = await invoke<TrafficAggregate[]>('query_traffic_stats', {
        params: {
          start: startTime,
          end: endTime,
          group_by: groupBy,
          limit: 100,
        },
      })
      return result
    },
    refetchInterval: 5000, // 每5秒刷新
  })

  // 排序后的数据
  const sortedData = useMemo(() => {
    if (!aggregateData) return []
    return [...aggregateData].sort((a, b) => {
      switch (sortBy) {
        case 'download':
          return b.total_download - a.total_download
        case 'upload':
          return b.total_upload - a.total_upload
        case 'total':
          return (b.total_upload + b.total_download) - (a.total_upload + a.total_download)
        case 'connections':
          return b.connection_count - a.connection_count
        default:
          return 0
      }
    })
  }, [aggregateData, sortBy])

  // 查询概览数据
  const { data: summaryData, isLoading: isSummaryLoading } = useQuery({
    queryKey: ['traffic-summary', timeRange],
    queryFn: async () => {
      const result = await invoke<TrafficSummary>('get_traffic_summary', {
        start: startTime,
        end: endTime,
      })
      return result
    },
    refetchInterval: 5000,
  })

  const isLoading = isAggregateLoading || isSummaryLoading

  return (
    <BasePage title={t('trafficStats.page.title')} header={<DataUsageRoundedIcon />}>
      <Grid container spacing={2}>
        {/* 概览卡片 */}
        <Grid size={{ xs: 12 }}>
          <Card>
            <CardContent>
              <Typography variant="h6" gutterBottom>
                {t('trafficStats.summary.title')}
              </Typography>
              {isSummaryLoading ? (
                <Skeleton variant="rectangular" height={80} />
              ) : (
                <Grid container spacing={2}>
                  <Grid size={{ xs: 6, sm: 4, md: 2.4 }}>
                    <StatCard
                      label={t('trafficStats.summary.totalUpload')}
                      value={formatTraffic(summaryData?.total_upload ?? 0)}
                    />
                  </Grid>
                  <Grid size={{ xs: 6, sm: 4, md: 2.4 }}>
                    <StatCard
                      label={t('trafficStats.summary.totalDownload')}
                      value={formatTraffic(summaryData?.total_download ?? 0)}
                    />
                  </Grid>
                  <Grid size={{ xs: 6, sm: 4, md: 2.4 }}>
                    <StatCard
                      label={t('trafficStats.summary.totalConnections')}
                      value={(summaryData?.total_connections ?? 0).toLocaleString()}
                    />
                  </Grid>
                  <Grid size={{ xs: 6, sm: 4, md: 2.4 }}>
                    <StatCard
                      label={t('trafficStats.summary.uniqueDomains')}
                      value={(summaryData?.unique_domains ?? 0).toLocaleString()}
                    />
                  </Grid>
                  <Grid size={{ xs: 6, sm: 4, md: 2.4 }}>
                    <StatCard
                      label={t('trafficStats.summary.uniqueProcesses')}
                      value={(summaryData?.unique_processes ?? 0).toLocaleString()}
                    />
                  </Grid>
                </Grid>
              )}
            </CardContent>
          </Card>
        </Grid>

        {/* 筛选控件 */}
        <Grid size={{ xs: 12 }}>
          <Card>
            <CardContent>
              <Box sx={{ display: 'flex', gap: 2, alignItems: 'center', flexWrap: 'wrap' }}>
                {/* 分组方式 */}
                <ToggleButtonGroup
                  value={groupBy}
                  exclusive
                  onChange={(_, value) => value && setGroupBy(value)}
                  size="small"
                >
                  <ToggleButton value="domain">
                    <PublicRoundedIcon sx={{ mr: 1 }} fontSize="small" />
                    {t('trafficStats.filter.byDomain')}
                  </ToggleButton>
                  <ToggleButton value="process">
                    <AppsRoundedIcon sx={{ mr: 1 }} fontSize="small" />
                    {t('trafficStats.filter.byProcess')}
                  </ToggleButton>
                </ToggleButtonGroup>

                {/* 时间范围 */}
                <FormControl size="small" sx={{ minWidth: 120 }}>
                  <InputLabel>{t('trafficStats.filter.timeRange')}</InputLabel>
                  <Select
                    value={timeRange}
                    label={t('trafficStats.filter.timeRange')}
                    onChange={(e) => setTimeRange(e.target.value as typeof timeRange)}
                  >
                    <MenuItem value="today">{t('trafficStats.filter.today')}</MenuItem>
                    <MenuItem value="week">{t('trafficStats.filter.thisWeek')}</MenuItem>
                    <MenuItem value="month">{t('trafficStats.filter.thisMonth')}</MenuItem>
                    <MenuItem value="all">{t('trafficStats.filter.allTime')}</MenuItem>
                  </Select>
                </FormControl>
              </Box>
            </CardContent>
          </Card>
        </Grid>

        {/* 数据表格 */}
        <Grid size={{ xs: 12 }}>
          <Card>
            <CardContent>
              <TableContainer component={Paper} variant="outlined">
                <Table size="small">
                  <TableHead>
                    <TableRow>
                      <TableCell>#</TableCell>
                      <TableCell>
                        {groupBy === 'domain'
                          ? t('trafficStats.table.domain')
                          : t('trafficStats.table.process')}
                      </TableCell>
                      <TableCell align="right">
                        <TableSortLabel
                          active={sortBy === 'upload'}
                          direction="desc"
                          onClick={() => setSortBy('upload')}
                          hideSortIcon={sortBy !== 'upload'}
                        >
                          {t('trafficStats.table.upload')}
                        </TableSortLabel>
                      </TableCell>
                      <TableCell align="right">
                        <TableSortLabel
                          active={sortBy === 'download'}
                          direction="desc"
                          onClick={() => setSortBy('download')}
                          hideSortIcon={sortBy !== 'download'}
                        >
                          {t('trafficStats.table.download')}
                        </TableSortLabel>
                      </TableCell>
                      <TableCell align="right">
                        <TableSortLabel
                          active={sortBy === 'total'}
                          direction="desc"
                          onClick={() => setSortBy('total')}
                          hideSortIcon={sortBy !== 'total'}
                        >
                          {t('trafficStats.table.total')}
                        </TableSortLabel>
                      </TableCell>
                      <TableCell align="right">
                        <TableSortLabel
                          active={sortBy === 'connections'}
                          direction="desc"
                          onClick={() => setSortBy('connections')}
                          hideSortIcon={sortBy !== 'connections'}
                        >
                          {t('trafficStats.table.connections')}
                        </TableSortLabel>
                      </TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {isLoading ? (
                      Array.from({ length: 5 }).map((_, i) => (
                        <TableRow key={i}>
                          <TableCell colSpan={6}>
                            <Skeleton />
                          </TableCell>
                        </TableRow>
                      ))
                    ) : sortedData && sortedData.length > 0 ? (
                      sortedData.map((row, index) => (
                        <TableRow key={row.key} hover>
                          <TableCell>{index + 1}</TableCell>
                          <TableCell
                            sx={{
                              maxWidth: 300,
                              overflow: 'hidden',
                              textOverflow: 'ellipsis',
                              whiteSpace: 'nowrap',
                            }}
                          >
                            {row.key}
                          </TableCell>
                          <TableCell align="right">{formatTraffic(row.total_upload)}</TableCell>
                          <TableCell align="right">{formatTraffic(row.total_download)}</TableCell>
                          <TableCell align="right">
                            {formatTraffic(row.total_upload + row.total_download)}
                          </TableCell>
                          <TableCell align="right">
                            {row.connection_count.toLocaleString()}
                          </TableCell>
                        </TableRow>
                      ))
                    ) : (
                      <TableRow>
                        <TableCell colSpan={6} align="center">
                          <Typography color="text.secondary" sx={{ py: 4 }}>
                            {t('trafficStats.table.noData')}
                          </Typography>
                        </TableCell>
                      </TableRow>
                    )}
                  </TableBody>
                </Table>
              </TableContainer>
            </CardContent>
          </Card>
        </Grid>
      </Grid>
    </BasePage>
  )
}

// 统计卡片组件
function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <Box sx={{ textAlign: 'center' }}>
      <Typography variant="h5" sx={{ fontWeight: 'bold' }}>
        {value}
      </Typography>
      <Typography variant="body2" color="text.secondary">
        {label}
      </Typography>
    </Box>
  )
}
