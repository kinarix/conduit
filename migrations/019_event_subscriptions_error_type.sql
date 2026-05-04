-- Allow 'error' event_type in event_subscriptions for BoundaryErrorEvent support.
ALTER TABLE event_subscriptions
    DROP CONSTRAINT event_subscriptions_event_type_check,
    ADD  CONSTRAINT event_subscriptions_event_type_check
         CHECK (event_type IN ('message', 'signal', 'error'));
