O workflow de release põe aqui o `lazyaicode-agent` Linux (compilado no
Ubuntu) antes de gerar o MSI. O app o copia para a distro WSL
(`~/.lazyaicode/lazyaicode-agent`) no primeiro uso. Em builds locais fica
vazio: defina `LAZYAICODE_AGENT_BIN` ou instale o agente na distro à mão.
