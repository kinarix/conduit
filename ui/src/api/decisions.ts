import { apiFetch } from './client'

export interface DecisionSummary {
  id: string
  decision_key: string
  version: number
  name: string | null
  deployed_at: string
  process_group_id: string | null
}

export interface InputClause {
  expression: string
}

export interface OutputClause {
  name: string
  output_values?: string[]
}

export interface Rule {
  input_entries: string[]
  output_entries: string[]
}

export type HitPolicy = 'UNIQUE' | 'FIRST' | 'COLLECT' | 'RULE_ORDER' | 'OUTPUT_ORDER' | 'ANY' | 'PRIORITY'
export type CollectAggregator = 'SUM' | 'MIN' | 'MAX' | 'COUNT'

export interface DecisionTable {
  hit_policy: HitPolicy
  collect_aggregator?: CollectAggregator
  inputs: InputClause[]
  outputs: OutputClause[]
  rules: Rule[]
  required_decisions: string[]
}

export interface DecisionDetail extends DecisionSummary {
  table: DecisionTable
}

export const fetchDecisions = (orgId: string, processGroupId?: string): Promise<DecisionSummary[]> => {
  const url = processGroupId
    ? `/api/v1/decisions?process_group_id=${encodeURIComponent(processGroupId)}`
    : `/api/v1/decisions`
  return apiFetch<DecisionSummary[]>(url, { headers: { 'X-Org-Id': orgId } })
}

export const fetchDecision = (orgId: string, key: string): Promise<DecisionDetail> =>
  apiFetch<DecisionDetail>(`/api/v1/decisions/${encodeURIComponent(key)}`, {
    headers: { 'X-Org-Id': orgId },
  })

export const deployDecision = (orgId: string, xml: string, processGroupId?: string): Promise<void> => {
  const url = processGroupId
    ? `/api/v1/decisions?process_group_id=${encodeURIComponent(processGroupId)}`
    : `/api/v1/decisions`
  return apiFetch<void>(url, {
    method: 'POST',
    headers: { 'X-Org-Id': orgId, 'Content-Type': 'text/xml' },
    body: xml,
  })
}

export interface TestResult {
  output?: Record<string, unknown>
  error?: string
  message?: string
}

export function nextDecisionName(decisions: DecisionSummary[]): string {
  const nums = decisions
    .map(d => d.name?.match(/^Decision (\d+)$/)?.[1])
    .filter((n): n is string => n !== undefined)
    .map(Number)
  return `Decision ${nums.length > 0 ? Math.max(...nums) + 1 : 1}`
}

export function makeStubDmn(key: string, name: string): string {
  return `<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" id="definitions_${key}" name="${name}" namespace="http://camunda.org/schema/1.0/dmn">
  <decision id="${key}" name="${name}">
    <decisionTable id="decisionTable_${key}">
      <input id="input_0" label="">
        <inputExpression id="inputExpr_0" typeRef="string">
          <text></text>
        </inputExpression>
      </input>
      <output id="output_0" label="" name="output" typeRef="string">
      </output>
      <rule id="rule_0">
        <inputEntry id="rule0_in0"><text></text></inputEntry>
        <outputEntry id="rule0_out0"><text></text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>`
}

export const deleteDecision = (orgId: string, key: string): Promise<void> =>
  apiFetch<void>(`/api/v1/decisions/${encodeURIComponent(key)}`, {
    method: 'DELETE',
    headers: { 'X-Org-Id': orgId },
  })

export const testDecision = (
  orgId: string,
  xml: string,
  context: Record<string, unknown>,
): Promise<TestResult> =>
  apiFetch<TestResult>(`/api/v1/decisions/test`, {
    method: 'POST',
    headers: { 'X-Org-Id': orgId },
    body: JSON.stringify({ dmn_xml: xml, context }),
  })
