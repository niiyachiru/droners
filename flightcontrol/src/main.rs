#![no_main]
#![no_std]
#![deny(warnings)]

use stm32f4xx_hal::dma;

extern crate cortex_m_semihosting;
extern crate panic_halt;

mod typedefs;

fn get_dshot_dma_cfg() -> dma::config::DmaConfig {
    dma::config::DmaConfig::default()
        .transfer_complete_interrupt(true)
        .transfer_error_interrupt(false)
        .half_transfer_interrupt(false)
        .fifo_enable(true)
        .fifo_threshold(dma::config::FifoThreshold::QuarterFull)
        .peripheral_burst(dma::config::BurstMode::NoBurst)
        .peripheral_increment(false)
        .memory_burst(dma::config::BurstMode::NoBurst)
        .memory_increment(true)
        .priority(dma::config::Priority::High)
}

#[rtic::app(device = stm32f4xx_hal::pac, peripherals = true)]
mod app {
    use core::mem;

    use droners_components::{e32, esc, neo6m};
    use esc::DSHOT_600_MHZ;
    use stm32f4xx_hal::{
        dma::{self, traits::Stream},
        pac,
        prelude::*,
        serial, stm32, timer,
    };

    use crate::get_dshot_dma_cfg;
    use crate::typedefs::*;

    #[resources]
    struct Resources {
        tim2: Timer2,

        serial1: Serial1,
        serial2: Serial2,

        dma_transfer4: DmaTransfer4,
        dma_transfer5: DmaTransfer5,
        dma_transfer7: DmaTransfer7,
        dma_transfer2: DmaTransfer2,

        esc1: Esc1,
        esc2: Esc2,
        esc3: Esc3,
        esc4: Esc4,

        controller_aux: ControllerAux,
        controller_m0: ControllerM0,
        controller_m1: ControllerM1,
        controller: Controller,

        gps: GpsModule,
    }

    #[init]
    fn init(cx: init::Context) -> init::LateResources {
        let _core: cortex_m::Peripherals = cx.core;
        let device: stm32::Peripherals = cx.device;

        let rcc = device.RCC.constrain();

        let clocks = rcc.cfgr.use_hse(25.mhz()).sysclk(48.mhz()).freeze();

        let gpioa = device.GPIOA.split();
        let gpiob = device.GPIOB.split();

        gpiob.pb4.into_alternate_af2();
        gpiob.pb5.into_alternate_af2();
        gpiob.pb0.into_alternate_af2();
        gpiob.pb1.into_alternate_af2();

        let mut tim2 = timer::Timer::tim2(device.TIM2, 750.hz(), clocks);
        tim2.listen(timer::Event::TimeOut);

        let serial1 = serial::Serial::usart1(
            device.USART1,
            (
                gpioa.pa9.into_alternate_af7(),
                gpioa.pa10.into_alternate_af7(),
            ),
            serial::config::Config::default()
                .baudrate(9600.bps())
                .parity_even()
                .stopbits(serial::config::StopBits::STOP1)
                .wordlength_8(),
            clocks,
        )
        .unwrap();

        let serial2 = serial::Serial::usart2(
            device.USART2,
            (
                gpioa.pa2.into_alternate_af7(),
                gpioa.pa3.into_alternate_af7(),
            ),
            serial::config::Config::default()
                .baudrate(9600.bps())
                .parity_even()
                .stopbits(serial::config::StopBits::STOP1)
                .wordlength_8(),
            clocks,
        )
        .unwrap();

        let dma1_streams = dma::StreamsTuple::new(device.DMA1);

        let ccr1_tim3 =
            dma::traits::CCR1::<pac::TIM3>(unsafe { mem::transmute_copy(&device.TIM3) });
        let ccr2_tim3 =
            dma::traits::CCR2::<pac::TIM3>(unsafe { mem::transmute_copy(&device.TIM3) });
        let ccr3_tim3 =
            dma::traits::CCR3::<pac::TIM3>(unsafe { mem::transmute_copy(&device.TIM3) });
        let ccr4_tim3 =
            dma::traits::CCR4::<pac::TIM3>(unsafe { mem::transmute_copy(&device.TIM3) });

        let dma_cfg = get_dshot_dma_cfg();

        let dma_transfer4: DmaTransfer4 = dma::Transfer::init(
            dma1_streams.4,
            ccr1_tim3,
            cortex_m::singleton!(: [u16; esc::DMA_BUFFER_LEN] = [0; esc::DMA_BUFFER_LEN]).unwrap(),
            None,
            dma_cfg,
        );

        let dma_transfer5: DmaTransfer5 = dma::Transfer::init(
            dma1_streams.5,
            ccr2_tim3,
            cortex_m::singleton!(: [u16; esc::DMA_BUFFER_LEN] = [0; esc::DMA_BUFFER_LEN]).unwrap(),
            None,
            dma_cfg,
        );

        let dma_transfer7: DmaTransfer7 = dma::Transfer::init(
            dma1_streams.7,
            ccr3_tim3,
            cortex_m::singleton!(: [u16; esc::DMA_BUFFER_LEN] = [0; esc::DMA_BUFFER_LEN]).unwrap(),
            None,
            dma_cfg,
        );

        let dma_transfer2: DmaTransfer2 = dma::Transfer::init(
            dma1_streams.2,
            ccr4_tim3,
            cortex_m::singleton!(: [u16; esc::DMA_BUFFER_LEN] = [0; esc::DMA_BUFFER_LEN]).unwrap(),
            None,
            dma_cfg,
        );

        let (esc1, esc2, esc3, esc4): (Esc1, Esc2, Esc3, Esc4) =
            esc::tim3(device.TIM3, clocks, DSHOT_600_MHZ.mhz());

        let controller_aux = gpioa.pa8.into_pull_up_input();
        let controller_m0 = gpiob.pb14.into_open_drain_output();
        let controller_m1 = gpiob.pb15.into_open_drain_output();
        let controller = e32::E32::<pac::USART1>::new();

        let gps = neo6m::Neo6m::<pac::USART2>::new();

        init::LateResources {
            tim2,

            serial1,
            serial2,

            dma_transfer4,
            dma_transfer5,
            dma_transfer7,
            dma_transfer2,

            esc1,
            esc2,
            esc3,
            esc4,

            controller_aux,
            controller_m0,
            controller_m1,
            controller,

            gps,
        }
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }

    #[task(binds = USART1, resources = [serial1, controller, esc1, esc2, esc3, esc4])]
    fn usart1(mut cx: usart1::Context) {
        let serial = &mut cx.resources.serial1;
        let controller = &mut cx.resources.controller;

        let msg = (serial, controller).lock(|serial: &mut Serial1, controller: &mut Controller| {
            while serial.is_rxne() {
                match controller.read(serial) {
                    Ok(msg) => {
                        return msg;
                    }
                    Err(_) => {
                        return None;
                    }
                }
            }

            None
        });

        match msg {
            Some(msg) => {
                use droners_components::e32::command::Command;

                match msg {
                    Command::Controller { right_trigger, .. } => {
                        let throttle = right_trigger as u16 * 256;

                        let esc1 = &mut cx.resources.esc1;
                        let esc2 = &mut cx.resources.esc2;
                        let esc3 = &mut cx.resources.esc3;
                        let esc4 = &mut cx.resources.esc4;

                        esc1.lock(|esc: &mut Esc1| {
                            esc.set_throttle(throttle);
                        });

                        esc2.lock(|esc: &mut Esc2| {
                            esc.set_throttle(throttle);
                        });

                        esc3.lock(|esc: &mut Esc3| {
                            esc.set_throttle(throttle);
                        });

                        esc4.lock(|esc: &mut Esc4| {
                            esc.set_throttle(throttle);
                        });
                    }
                }
            }
            None => {}
        }
    }

    #[task(binds = USART2, resources = [serial2, gps])]
    fn usart2(mut cx: usart2::Context) {
        let serial = &mut cx.resources.serial2;
        let gps = &mut cx.resources.gps;

        (serial, gps).lock(
            |serial: &mut Serial2, gps: &mut GpsModule| {
                if let Some(_) = gps.read(serial) {}
            },
        )
    }

    #[task(binds = TIM2, priority = 2, resources = [tim2, dma_transfer4, dma_transfer5, dma_transfer7, dma_transfer2, esc1, esc2, esc3, esc4])]
    fn tim2(mut cx: tim2::Context) {
        let tim = &mut cx.resources.tim2;
        let transfer4 = &mut cx.resources.dma_transfer4;
        let transfer5 = &mut cx.resources.dma_transfer5;
        let transfer7 = &mut cx.resources.dma_transfer7;
        let transfer2 = &mut cx.resources.dma_transfer2;
        let esc1 = &mut cx.resources.esc1;
        let esc2 = &mut cx.resources.esc2;
        let esc3 = &mut cx.resources.esc3;
        let esc4 = &mut cx.resources.esc4;

        tim.lock(|tim: &mut Timer2| {
            tim.clear_interrupt(timer::Event::TimeOut);
        });

        (transfer4, esc1).lock(|transfer: &mut DmaTransfer4, esc: &mut Esc1| {
            esc.start(transfer);
        });

        (transfer5, esc2).lock(|transfer: &mut DmaTransfer5, esc: &mut Esc2| {
            esc.start(transfer);
        });

        (transfer7, esc3).lock(|transfer: &mut DmaTransfer7, esc: &mut Esc3| {
            esc.start(transfer);
        });

        (transfer2, esc4).lock(|transfer: &mut DmaTransfer2, esc: &mut Esc4| {
            esc.start(transfer);
        });

        unsafe {
            let tim3 = pac::TIM3::ptr();
            (*tim3).cnt.modify(|_, w| w.bits(0));
            (*tim3).dier.modify(|_, w| w.cc1de().enabled());
            (*tim3).dier.modify(|_, w| w.cc2de().enabled());
            (*tim3).dier.modify(|_, w| w.cc3de().enabled());
            (*tim3).dier.modify(|_, w| w.cc4de().enabled());
        }
    }

    #[task(binds = DMA1_STREAM4, priority = 3, resources = [dma_transfer4, esc1])]
    fn dma1_stream4(mut cx: dma1_stream4::Context) {
        let transfer = &mut cx.resources.dma_transfer4;
        let esc = &mut cx.resources.esc1;

        if !dma::Stream4::<pac::DMA1>::get_transfer_complete_flag() {
            return;
        }

        transfer.lock(|transfer| {
            transfer.clear_transfer_complete_interrupt();

            esc.lock(|esc| {
                esc.pause(transfer);
            })
        });
    }

    #[task(binds = DMA1_STREAM5, priority = 3, resources = [dma_transfer5, esc2])]
    fn dma1_stream5(mut cx: dma1_stream5::Context) {
        let transfer = &mut cx.resources.dma_transfer5;
        let esc = &mut cx.resources.esc2;

        if !dma::Stream5::<pac::DMA1>::get_transfer_complete_flag() {
            return;
        }

        transfer.lock(|transfer| {
            transfer.clear_transfer_complete_interrupt();

            esc.lock(|esc| {
                esc.pause(transfer);
            })
        });
    }

    #[task(binds = DMA1_STREAM7, priority = 3, resources = [dma_transfer7, esc3])]
    fn dma1_stream7(mut cx: dma1_stream7::Context) {
        let transfer = &mut cx.resources.dma_transfer7;
        let esc = &mut cx.resources.esc3;

        if !dma::Stream7::<pac::DMA1>::get_transfer_complete_flag() {
            return;
        }

        transfer.lock(|transfer| {
            transfer.clear_transfer_complete_interrupt();

            esc.lock(|esc| {
                esc.pause(transfer);
            })
        });
    }

    #[task(binds = DMA1_STREAM2, priority = 3, resources = [dma_transfer2, esc4])]
    fn dma1_stream2(mut cx: dma1_stream2::Context) {
        let transfer = &mut cx.resources.dma_transfer2;
        let esc = &mut cx.resources.esc4;

        if !dma::Stream2::<pac::DMA1>::get_transfer_complete_flag() {
            return;
        }

        transfer.lock(|transfer| {
            transfer.clear_transfer_complete_interrupt();

            esc.lock(|esc| {
                esc.pause(transfer);
            })
        });
    }
}
