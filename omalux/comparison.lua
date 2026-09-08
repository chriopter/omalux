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
  local applied, last_error
  local style_revision, controls_applied, imported = 0, {}, {}
  while not dt.control.ending do
    local file = io.open(mailbox, "r")
    local command = file and file:read("*a")
    if file then file:close() end
    if command and command ~= applied and dt.gui.current_view() == dt.gui.views.darkroom then
      local ok, err = pcall(function()
        local body = assert(command:match("^omalux%-controls%-v3 %d+\n(.*)$"), "unsupported controls message")
        local records = {}
        for line in body:gmatch("[^\n]+") do
          local sequence, filename, name = line:match("^style (%d+) (%S+) (%S+)$")
          if sequence then
            filename, name = decode(filename), decode(name)
            assert(not filename:find("[/\\]") and filename:match("%.dtstyle$"), "invalid style filename")
            records[#records+1] = {kind="style", sequence=tonumber(sequence), filename=filename, name=name}
          else
            local epoch, module, parameter, value, revision = line:match("^control (%d+) ([%w_]+) ([%w_]+) ([%d.eE+%-]+) (%d+)$")
            value = tonumber(value)
            assert(epoch and value and value == value and math.abs(value) < math.huge, "invalid control")
            records[#records+1] = {kind="control", epoch=tonumber(epoch), module=module, parameter=parameter, value=value, revision=tonumber(revision)}
          end
        end
        for _, record in ipairs(records) do
          if record.kind == "style" then
            if record.sequence > style_revision then
              if not imported[record.filename] then
                local previous = find_style(record.name)
                if previous then dt.styles.delete(previous) end
                dt.styles.import(assert(os.getenv("OMALUX_PRESETS_DIR")) .. "/" .. record.filename)
                imported[record.filename] = true
              end
              local selected = assert(find_style(record.name), "style import failed: " .. record.name)
              local image = assert(dt.gui.views.darkroom.display_image(), "no darkroom image")
              dt.styles.apply(selected, image)
              style_revision, controls_applied = record.sequence, {}
              log("applied style " .. record.name)
            end
          elseif record.epoch == style_revision then
            local key = record.module .. "/" .. record.parameter
            if record.revision > (controls_applied[key] or 0) then
              local enabled = dt.gui.action("iop/" .. record.module, "enable", "on", 1)
              local result = dt.gui.action("iop/" .. key, "value", "set", record.value)
              assert(enabled == enabled and result == result, "control unavailable: " .. key)
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
