// Minimal valid BPMN skeleton used as the initial bpmn_xml when a draft is
// created via the "name first" flow. Matches the shape produced by toXml so
// fromXml can round-trip it.
export function defaultBpmnXml(processKey: string, processName: string): string {
  const escAttr = (s: string) =>
    s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')

  return `<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" xmlns:bpmndi="http://www.omg.org/spec/BPMN/20100524/DI" xmlns:dc="http://www.omg.org/spec/DD/20100524/DC" id="Definitions_1" targetNamespace="http://conduit.io/bpmn">
  <process id="${escAttr(processKey)}" name="${escAttr(processName)}" isExecutable="true">
    <startEvent id="start_1" name="Start"/>
  </process>
  <bpmndi:BPMNDiagram id="BPMNDiagram_1">
    <bpmndi:BPMNPlane id="BPMNPlane_1" bpmnElement="${escAttr(processKey)}">
      <bpmndi:BPMNShape id="BPMNShape_start_1" bpmnElement="start_1">
        <dc:Bounds x="80" y="180" width="36" height="36"/>
      </bpmndi:BPMNShape>
    </bpmndi:BPMNPlane>
  </bpmndi:BPMNDiagram>
</definitions>
`
}
