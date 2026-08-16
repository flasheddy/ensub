begin;

do $$
begin
  if exists (select 1 from public.vocabulary_records limit 1) then
    raise exception 'vocabulary_records must be empty before owner isolation is enabled';
  end if;
end
$$;

alter table public.vocabulary_records
  add column user_id uuid;

alter table public.vocabulary_records
  alter column user_id set not null,
  alter column confidence type double precision using confidence::double precision,
  alter column created_at set default now(),
  alter column created_at set not null;

alter table public.vocabulary_records
  add constraint vocabulary_records_user_id_fkey
    foreign key (user_id) references auth.users(id) on delete cascade,
  add constraint vocabulary_records_target_phrase_length
    check (char_length(btrim(target_phrase)) between 1 and 120),
  add constraint vocabulary_records_target_sentence_length
    check (char_length(btrim(target_sentence)) between 1 and 2000),
  add constraint vocabulary_records_surrounding_context_length
    check (surrounding_context is null or char_length(surrounding_context) <= 5000),
  add constraint vocabulary_records_lemma_length
    check (char_length(btrim(lemma)) between 1 and 120),
  add constraint vocabulary_records_part_of_speech_length
    check (char_length(btrim(part_of_speech)) between 1 and 80),
  add constraint vocabulary_records_definition_length
    check (char_length(btrim(definition)) between 1 and 2000),
  add constraint vocabulary_records_nuance_length
    check (char_length(btrim(nuance)) between 1 and 2000),
  add constraint vocabulary_records_confidence_range
    check (confidence between 0 and 1);

create index vocabulary_records_user_created_id_idx
  on public.vocabulary_records (user_id, created_at desc, id desc);

alter table public.vocabulary_records enable row level security;

drop policy if exists "Allow public read/insert" on public.vocabulary_records;
drop policy if exists "Vocabulary owners can read" on public.vocabulary_records;
drop policy if exists "Vocabulary owners can insert" on public.vocabulary_records;

create policy "Vocabulary owners can read"
  on public.vocabulary_records
  for select
  to authenticated
  using ((select auth.uid()) = user_id);

create policy "Vocabulary owners can insert"
  on public.vocabulary_records
  for insert
  to authenticated
  with check ((select auth.uid()) = user_id);

revoke all privileges on table public.vocabulary_records from public, anon, authenticated;
grant select, insert on table public.vocabulary_records to authenticated;

commit;
