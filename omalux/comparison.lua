-- SPDX-License-Identifier: GPL-3.0-or-later
-- Complete style boundaries plus coalesced slider snapshots, in application order.
local dt = require "darktable"
local mailbox = assert(os.getenv("OMALUX_COMPARISON_MAILBOX"))
local function log(message)
  io.stderr:write("[omalux comparison] " .. message .. "\n")
  io.stderr:flush()
end
local function decode(value)
  return (value:gsub("%%(%x%x)", function(hex) return string.char(tonumber(hex, 16)) end))
end
local function find_style(name)
  for _, style in ipairs(dt.styles) do if style.name == name then return style end end
end

dt.control.dispatch(function()
  log("bridge ready")
  local applied, last_error, current_source
  local style_revision, controls_applied, imported = 0, {}, {}
  while not dt.control.ending do
    local file = io.open(mailbox, "r")
    local command = file and file:read("*a")
    if file then file:close() end
    if command and command ~= applied and dt.gui.current_view() == dt.gui.views.darkroom then
      local ok, err = pcall(function()
        local body = assert(command:match("^omalux%-controls%-v3 %d+\n(.*)$"), "unsupported controls message")
        local source, rest = body:match("^source (%S+)\n(.*)$")
        if source then
          source=decode(source);body=rest
          if source ~= current_source then
            local image=assert(dt.database.import(source), "Could not import comparison image")
            local shown=dt.gui.views.darkroom.display_image()
            if not shown or shown.id ~= image.id then
              dt.gui.views.darkroom.display_image(image)
              -- The setter schedules a GTK idle image load; do not edit the old image.
              repeat
                dt.control.sleep(50)
                shown=dt.gui.views.darkroom.display_image()
              until dt.control.ending or (shown and shown.id == image.id)
            end
            current_source=source;style_revision=0;controls_applied={}
          end
        end
        local records = {}
        for line in body:gmatch("[^\n]+") do
          local history_epoch, history_file = line:match("^history (%d+) ([%w%-]+)$")
          if history_epoch then
            records[#records+1]={kind="history",sequence=tonumber(history_epoch),filename=history_file}
          else
          local epoch, revision, module, snapshot = line:match("^module (%d+) (%d+) ([%w_]+) ([%w%-]+)$")
          if epoch then
            records[#records+1]={kind="module",epoch=tonumber(epoch),revision=tonumber(revision),module=module,name=snapshot}
          else
          local sequence, filename, name = line:match("^style (%d+) (%S+) (%S+)$")
          if sequence then
            filename, name = decode(filename), decode(name)
            assert(not filename:find("[\\%z]") and not filename:match("^/") and not filename:find("//", 1, true)
              and filename:match("%.dtstyle$"), "invalid style filename")
            for segment in filename:gmatch("[^/]+") do
              assert(segment ~= "." and segment ~= "..", "invalid style path")
            end
            records[#records+1] = {kind="style", sequence=tonumber(sequence), filename=filename, name=name}
          else
            local epoch, module, parameter, value, revision = line:match("^control (%d+) ([%w_]+) (%S+) ([%d.eE+%-]+) (%d+)$")
            value = tonumber(value)
            assert(epoch and value and value == value and math.abs(value) < math.huge, "invalid control")
            records[#records+1] = {kind="control", epoch=tonumber(epoch), module=module, parameter=decode(parameter), value=value, revision=tonumber(revision)}
          end
        end
          end
        end
        for _, record in ipairs(records) do
          if record.kind == "history" then
            if record.sequence > style_revision then
              local previous=find_style(record.filename)
              if previous then dt.styles.delete(previous) end
              dt.styles.import(mailbox:match("^(.*)/") .. "/" .. record.filename .. ".dtstyle")
              local selected=assert(find_style(record.filename), "history snapshot import failed")
              dt.styles.apply(selected,assert(dt.gui.views.darkroom.display_image(), "no darkroom image"))
              style_revision,controls_applied=record.sequence,{}
              log("restored history " .. record.sequence)
            end
          elseif record.kind == "style" then
            if record.sequence > style_revision then
              if not imported[record.filename] then
                local previous = find_style(record.name)
                if previous then dt.styles.delete(previous) end
                dt.styles.import(assert(os.getenv("OMALUX_STYLES_DIR")) .. "/" .. record.filename)
                imported[record.filename] = true
              end
              local selected = assert(find_style(record.name), "style import failed: " .. record.name)
              local image = assert(dt.gui.views.darkroom.display_image(), "no darkroom image")
              dt.styles.apply(selected, image)
              style_revision, controls_applied = record.sequence, {}
              log("applied style " .. record.name)
            end
          elseif record.kind == "module" and record.epoch == style_revision then
            local key="recipe/" .. record.module
            if record.revision > (controls_applied[key] or 0) then
              local previous=find_style(record.name); if previous then dt.styles.delete(previous) end
              dt.styles.import(mailbox:match("^(.*)/") .. "/" .. record.name .. ".dtstyle")
              local style=assert(find_style(record.name), "module snapshot import failed")
              dt.styles.apply(style, assert(dt.gui.views.darkroom.display_image()))
              controls_applied[key]=record.revision
            end
          elseif record.kind == "control" and record.epoch == style_revision then
            local key = record.module .. "/" .. record.parameter
            if record.revision > (controls_applied[key] or 0) then
              if record.parameter == "@enabled" then
                dt.gui.action("iop/" .. record.module, "enable", record.value > .5 and "on" or "off", 1)
              else
              local enabled = dt.gui.action("iop/" .. record.module, "enable", "on", 1)
              local result = dt.gui.action("iop/" .. key, "value", "set", record.value)
              assert(enabled == enabled and result == result, "control unavailable: " .. key)
              end
              controls_applied[key] = record.revision
            end
          end
        end
      end)
      if ok then
        applied, last_error = command, nil
        log("applied revision " .. command:match("^omalux%-controls%-v3 (%d+)"))
      elseif tostring(err) ~= last_error then
        last_error = tostring(err)
        log(last_error)
      end
    end
    dt.control.sleep(50)
  end
end)
