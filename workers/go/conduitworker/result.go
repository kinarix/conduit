package conduitworker

// ResultKind classifies how a Handler reports back to the engine.
type ResultKind int

const (
	// ResultComplete reports POST /complete with optional output variables.
	ResultComplete ResultKind = iota
	// ResultBpmnError reports POST /bpmn-error with a domain error code that
	// branches the BPMN through a boundaryErrorEvent.
	ResultBpmnError
)

// Result is what a Handler returns. Returning a non-nil error from a Handler
// triggers POST /failure (transient retry) instead of using a Result.
type Result struct {
	Kind         ResultKind
	Variables    []Variable
	ErrorCode    string
	ErrorMessage string
}

// Complete builds a Result that reports the task as completed with the given
// output variables.
func Complete(vars ...Variable) *Result {
	return &Result{Kind: ResultComplete, Variables: vars}
}

// BpmnError builds a Result that throws a BPMN error with the given code.
// `errorMessage` is informational and shown in the engine's error log.
// `vars` are written to the instance before the error propagates.
func BpmnError(code, errorMessage string, vars ...Variable) *Result {
	return &Result{
		Kind:         ResultBpmnError,
		ErrorCode:    code,
		ErrorMessage: errorMessage,
		Variables:    vars,
	}
}
